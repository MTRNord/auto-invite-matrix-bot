use std::convert::TryFrom;

use clap::Clap;
use futures_util::future::join_all;
use futures_util::stream::TryStreamExt as _;
use ruma_client::api::r0::membership::invite_user;
use ruma_client::api::r0::membership::join_room_by_id;
use ruma_client::api::r0::message::create_message_event;
use ruma_client::api::r0::sync::sync_events::IncomingResponse;
use ruma_client::identifiers::UserId;
use ruma_client::{
    self,
    events::{
        room::message::{MessageEventContent, NoticeMessageEventContent},
        EventType,
    },
    HttpsClient, Session,
};
use url::Url;

use log::{debug, error, info};

use crate::config::{load_config, Config, Homeserver};
use crate::logger::setup_logger;

mod config;
mod logger;

async fn do_stuff(config: &Config, server: &Homeserver) -> Result<(), ruma_client::Error> {
    info!("Starting session as {}", server.mxid);

    let session: Session;
    let mut client: HttpsClient = HttpsClient::https(
        Url::parse(server.address.as_str()).expect("unable to parse url"),
        None,
    );
    if server.access_token.is_some() {
        session = Session {
            device_id: "Ruma Bot".to_string(),
            access_token: server
                .access_token
                .as_ref()
                .expect("missing access_token")
                .to_owned(),
            user_id: UserId::try_from(server.mxid.as_str()).unwrap(),
        };

        client = HttpsClient::https(
            Url::parse(server.address.as_str()).expect("unable to parse url"),
            Some(session),
        );
    } else if server.password.is_some() {
        client
            .log_in(
                server.mxid.clone(),
                server
                    .password
                    .as_ref()
                    .expect("failed to read password")
                    .to_owned(),
                None,
                None,
            )
            .await?;
    } else {
        error!("Please provide either a password or an access_token!");
    }

    let mut sync_stream = Box::pin(client.sync(None, None, false));
    let message = config.message.clone();
    let target_user = config.target_user.clone();
    while let Some(response) = sync_stream.try_next().await? {
        let res: IncomingResponse = response;
        for (room_id, _room) in res.rooms.invite {
            // Auto join rooms
            debug!("Invited to {:?}", room_id.clone());
            client
                .request(join_room_by_id::Request {
                    room_id: room_id.clone(),
                    third_party_signed: None,
                })
                .await?;
            debug!("Joined {:?}", room_id.clone());
            let response = client
                .request(invite_user::Request {
                    room_id: room_id.clone(),
                    user_id: UserId::try_from(target_user.clone().as_str()).unwrap(),
                })
                .await?;
            debug!("Invited correct user {:?}", response);
            client
                .request(create_message_event::Request {
                    room_id,
                    event_type: EventType::RoomMessage,
                    txn_id: "1".to_owned(),
                    data: MessageEventContent::Notice(NoticeMessageEventContent {
                        body: message.clone(),
                        relates_to: None,
                    }),
                })
                .await?;

            debug!("Sent a message about what happened");
        }
    }

    Ok(())
}

#[derive(Clap)]
#[clap(version = "0.1.0", author = "MTRNord")]
struct Opts {
    #[clap(short = "c", long = "config", default_value = "config.yaml")]
    config: String,
    /// A level of verbosity, and can be used multiple times
    #[clap(short = "v", long = "verbose", parse(from_occurrences))]
    verbose: i32,
}

#[tokio::main]
async fn main() -> Result<(), ruma_client::Error> {
    let opts: Opts = Opts::parse();

    let log_level = match opts.verbose {
        0 => log::LevelFilter::Info,  // default
        1 => log::LevelFilter::Error, // -v
        2 => log::LevelFilter::Warn,  // -vv
        3 => log::LevelFilter::Debug, // -vvv
        _ => log::LevelFilter::Trace, // -vvvv and above
    };

    setup_logger(log_level).expect("unable to setup logger");

    let config = load_config(opts.config).expect("unable to read config");
    let mut futures = vec![];

    config.servers.iter().for_each(|x| {
        futures.push(do_stuff(&config, x));
    });

    join_all(futures).await;
    Ok(())
}
