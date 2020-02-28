use std::fs::File;
use std::io::BufReader;

use serde::{Deserialize, Serialize};

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Config {
    pub message: String,
    pub target_user: String,
    pub servers: Vec<Homeserver>,
    pub debug: bool,
}

#[derive(Debug, PartialEq, Serialize, Deserialize)]
pub struct Homeserver {
    pub address: String,
    pub mxid: String,
    pub access_token: Option<String>,
    pub password: Option<String>,
}

pub fn load_config(config_file: String) -> Result<Config, serde_yaml::Error> {
    let f = File::open(config_file).expect("Unable to open config file");
    let br = BufReader::new(f);

    let deserialized_config: Config = serde_yaml::from_reader(br)?;
    Ok(deserialized_config)
}
