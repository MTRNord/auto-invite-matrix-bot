use std::convert::TryFrom;
use std::fs;
use std::fs::File;
use std::io::{BufReader, BufWriter, Read, Write};

use clap::Clap;
use futures_util::stream::FuturesUnordered;
use futures_util::stream::TryStreamExt as _;
use log::{debug, error, info, warn};
use rand::distributions::Alphanumeric;
use rand::{thread_rng, Rng};
use ruma_client::api::r0::membership::invite_user;
use ruma_client::api::r0::membership::join_room_by_id;
use ruma_client::api::r0::message::create_message_event;
use ruma_client::api::r0::sync::sync_events::IncomingResponse;
use ruma_client::identifiers::UserId;
use ruma_client::{
    self,
    api::r0::{
        filter::{FilterDefinition, RoomEventFilter, RoomFilter},
        message::create_message_event::Response,
        room::create_room,
        sync::sync_events::Filter,
    },
    events::{
        collections::all::RoomEvent::{self, RoomMessage},
        room::message::{
            InReplyTo, MessageEvent,
            MessageEventContent::{self, Text},
            NoticeMessageEventContent, RelatesTo, TextMessageEventContent,
        },
        EventType,
    },
    identifiers::{EventId, RoomId},
    HttpsClient, Session,
};
use url::Url;

use crate::config::{load_config, Config, Homeserver};
use crate::logger::setup_logger;

mod config;
mod logger;

async fn send_notice_reply(
    client: &HttpsClient,
    text: String,
    related_event: EventId,
    room_id: RoomId,
) -> Result<Response, ruma_client::Error> {
    let rand_string: String = thread_rng().sample_iter(&Alphanumeric).take(30).collect();
    client
        .request(create_message_event::Request {
            room_id,
            event_type: EventType::RoomMessage,
            txn_id: rand_string,
            data: MessageEventContent::Notice(NoticeMessageEventContent {
                body: text,
                relates_to: Some(RelatesTo {
                    in_reply_to: InReplyTo {
                        event_id: related_event,
                    },
                }),
            }),
        })
        .await
}

async fn send_message(
    client: &HttpsClient,
    text: String,
    room_id: RoomId,
) -> Result<Response, ruma_client::Error> {
    let rand_string: String = thread_rng().sample_iter(&Alphanumeric).take(30).collect();
    client
        .request(create_message_event::Request {
            room_id,
            event_type: EventType::RoomMessage,
            txn_id: rand_string,
            data: MessageEventContent::Text(TextMessageEventContent {
                body: text,
                format: None,
                formatted_body: None,
                relates_to: None,
            }),
        })
        .await
}

async fn get_control_room(
    client: &HttpsClient,
    target_user: String,
) -> Result<RoomId, failure::Error> {
    fs::create_dir_all("./tmp/")?;
    let mut room_id_read = String::new();
    match File::open(format!(
        "./tmp/control_room_{}",
        client.session().unwrap().user_id.hostname()
    )) {
        Ok(f) => {
            let mut br = BufReader::new(f);
            match br.read_to_string(&mut room_id_read) {
                Ok(_) => {
                    let room_id = RoomId::try_from(room_id_read.as_str()).unwrap();
                    Ok(room_id)
                }
                Err(e) => Err(failure::Error::try_from(e)?),
            }
        }
        Err(e) => {
            warn!("Unable to open control_room_tmp: {}", e);
            let resp = client
                .request(create_room::Request {
                    invite: vec![UserId::try_from(target_user.as_str())?],
                    name: Some(format!(
                        "AutoInviteBot Control Room {}",
                        client.session().unwrap().user_id.hostname()
                    )),
                    preset: Some(create_room::RoomPreset::TrustedPrivateChat),
                    creation_content: None,
                    room_alias_name: None,
                    topic: None,
                    visibility: None,
                })
                .await?;
            let room_id: RoomId = resp.room_id;
            let f = File::create(format!(
                "./tmp/control_room_{}",
                client.session().unwrap().user_id.hostname()
            ))?;
            let mut f = BufWriter::new(f);
            f.write_all(room_id.to_string().as_bytes())?;

            Ok(room_id)
        }
    }
}

async fn get_client(
    server: &Homeserver,
    target_user: String,
) -> Result<(HttpsClient, RoomId), failure::Error> {
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

    let control_room_id = get_control_room(&client, target_user).await?;

    info!("Started session as {}", server.mxid);

    Ok((client, control_room_id))
}

async fn parse_invites(
    client: &HttpsClient,
    room_id: RoomId,
    target_user: String,
    message: String,
) -> Result<(), ruma_client::Error> {
    // Auto join rooms
    debug!("Invited to {:?}", room_id);
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
            user_id: UserId::try_from(target_user.as_str()).unwrap(),
        })
        .await?;
    debug!("Invited correct user {:?}", response);

    let rand_string: String = thread_rng().sample_iter(&Alphanumeric).take(30).collect();
    client
        .request(create_message_event::Request {
            room_id: room_id.clone(),
            event_type: EventType::RoomMessage,
            txn_id: rand_string,
            data: MessageEventContent::Notice(NoticeMessageEventContent {
                body: message,
                relates_to: None,
            }),
        })
        .await?;

    debug!("Sent a message about what happened");
    Ok(())
}

async fn handle_mention(
    client: &HttpsClient,
    config: &Config,
    control_room_id: RoomId,
    room_id: RoomId,
    sender: UserId,
    event_id: EventId,
    body: String,
) -> Result<(), failure::Error> {
    if config.debug {
        send_notice_reply(
            &client,
            format!(
                "> <{}> {}\n\n[DEBUG] Mentioned main account about this mention",
                sender, body
            ),
            event_id.clone(),
            room_id.clone(),
        )
        .await?;
    }

    // Send mention to control room
    send_message(
        &client,
        format!(
            "> <{}> {}\n\n Mention in {} by {}",
            sender, body, room_id, sender
        ),
        control_room_id.clone(),
    )
    .await?;

    Ok(())
}

async fn parse_joins(
    client: &HttpsClient,
    config: &Config,
    server: &Homeserver,
    event: RoomEvent,
    room_id: RoomId,
    control_room_id: RoomId,
) -> Result<(), failure::Error> {
    if let RoomMessage(msg) = event {
        let content = msg.content;
        let event_id = msg.event_id;
        let sender = msg.sender;
        if let Text(msg_content) = content {
            let formatted_body: Option<String> = msg_content.formatted_body;
            let body: String = msg_content.body;

            if sender.to_string() != client.session().unwrap().user_id.to_string()
                && formatted_body.clone().is_some()
                && formatted_body
                    .clone()
                    .unwrap()
                    .to_lowercase()
                    .contains(&server.mxid.clone().to_lowercase())
            {
                handle_mention(
                    client,
                    config,
                    control_room_id.clone(),
                    room_id.clone(),
                    sender.clone(),
                    event_id.clone(),
                    body.clone(),
                )
                .await?;
            } else if sender.to_string() != client.session().unwrap().user_id.to_string()
                && formatted_body.clone().is_none()
                && body.to_lowercase().contains(
                    server
                        .mxid
                        .clone()
                        .to_lowercase()
                        .split(':')
                        .collect::<Vec<_>>()[0],
                )
            {
                handle_mention(
                    client,
                    config,
                    control_room_id.clone(),
                    room_id.clone(),
                    sender.clone(),
                    event_id.clone(),
                    body.clone(),
                )
                .await?;
            }
            // Todo make a smarter command handler
            /*if body.starts_with("!test") {
                send_notice_reply(
                    &client,
                    format!("> <{}> {}\n\ntest resp", sender, body),
                    event_id.clone(),
                    room_id.clone(),
                )
                .await?;
            }*/
        }
    }
    Ok(())
}

async fn load_next_batch() -> Result<String, failure::Error> {
    fs::create_dir_all("./tmp/")?;
    let mut next_batch = String::new();
    let f = File::open("./tmp/next_batch")?;
    let mut br = BufReader::new(f);
    br.read_to_string(&mut next_batch)?;
    Ok(next_batch)
}

async fn save_next_batch(next_batch: String) -> Result<(), failure::Error> {
    let f = File::create("./tmp/next_batch")?;
    let mut f = BufWriter::new(f);
    f.write_all(next_batch.as_bytes())?;
    Ok(())
}

fn generate_filter() -> Result<Filter, failure::Error> {
    let filter = FilterDefinition {
        event_fields: None,
        event_format: None,
        account_data: None,
        room: Some(RoomFilter {
            include_leave: None,
            account_data: None,
            timeline: Some(RoomEventFilter {
                not_types: vec![],
                not_rooms: vec![],
                limit: None,
                rooms: None,
                not_senders: vec![],
                senders: None,
                types: Some(vec!["m.room.message".to_owned()]),
                contains_url: None,
            }),
            ephemeral: None,
            state: None,
            not_rooms: vec![],
            rooms: None,
        }),
        presence: None,
    };
    Ok(Filter::FilterDefinition(filter))
}

async fn do_stuff(config: &Config, server: &Homeserver) -> Result<(), failure::Error> {
    let target_user = config.target_user.clone();
    let (client, control_room_id) = get_client(server, target_user.clone()).await?;

    let since = match load_next_batch().await {
        Ok(v) => Some(v),
        Err(e) => {
            error!("{:?}", e);
            None
        }
    };

    let filter = match generate_filter() {
        Ok(v) => Some(v),
        Err(e) => {
            error!("{:?}", e);
            None
        }
    };

    let mut sync_stream = Box::pin(client.sync(filter, since, false));
    let message = config.message.clone();
    while let Some(response) = sync_stream.try_next().await? {
        let res: IncomingResponse = response;
        let next_batch: String = res.next_batch;
        save_next_batch(next_batch).await?;

        for (room_id, _room) in res.rooms.invite {
            parse_invites(
                &client,
                room_id.clone(),
                target_user.clone(),
                message.clone(),
            )
            .await?;
        }
        for (room_id, room) in res.rooms.join {
            let events = room.timeline.events;

            for event in events.into_iter().flat_map(|r| r.into_result()) {
                if let RoomEvent::RoomMessage(MessageEvent { .. }) = event {
                    parse_joins(
                        &client,
                        config,
                        server,
                        event.clone(),
                        room_id.clone(),
                        control_room_id.clone(),
                    )
                    .await?;
                }
            }
        }
    }

    Ok(())
}

#[derive(Clap)]
#[clap(version = "0.1.0", author = "MTRNord")]
struct Opts {
    #[clap(short = 'c', long = "config", default_value = "config.yaml")]
    config: String,
    /// A level of verbosity, and can be used multiple times
    #[clap(short = 'v', long = "verbose", parse(from_occurrences))]
    verbose: i32,
}

#[tokio::main]
async fn main() -> Result<(), failure::Error> {
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

    let mut futures = FuturesUnordered::new();

    config.servers.iter().for_each(|x| {
        futures.push(do_stuff(&config, x));
    });

    while let Some(x) = futures.try_next().await? {
        error!("{:?}", x);
    }

    Ok(())
}
