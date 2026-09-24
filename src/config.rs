use serde::Deserialize;
use std::fs;



#[derive(Debug, Deserialize, Clone)]
pub struct Config {

    pub homeassistant: HomeAssistantConfig,

    pub logging: LoggingConfig,

    pub system: SystemConfig,

    pub command: CommandConfig,

}



#[derive(Debug, Deserialize, Clone)]
pub struct HomeAssistantConfig {

    pub url: String,

    pub token: String,

}



#[derive(Debug, Deserialize, Clone)]
pub struct LoggingConfig {

    pub level: String,

}



#[derive(Debug, Deserialize, Clone)]
pub struct SystemConfig {

    pub mode: String,

}



#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq,
    Hash,
    PartialOrd,
    Ord,
    Deserialize
)]
pub enum RunMode {

    Development,

    Production,

}



#[derive(Debug, Deserialize, Clone)]
pub struct CommandConfig {

    pub confirmation_timeout: u64,

}



impl Config {


    pub fn load() -> Self {


        let content =
            fs::read_to_string(
                "config.toml"
            )
            .expect(
                "Cannot read config.toml"
            );



        toml::from_str(
            &content
        )
        .expect(
            "Invalid config.toml"
        )

    }

}