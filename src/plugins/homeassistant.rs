use anyhow::Result;
use crate::ha::connection_state::ConnectionState;
use async_trait::async_trait;
use crate::logger::info;
use crate::ha::client::HAClient;
use crate::plugins::state::PluginState;
use crate::plugins::interface::{
    Plugin,
    PluginResponse,
};
use crate::plugins::instance::PluginInstance;
use crate::plugins::manifest::PluginManifest;
use crate::Config;
use std::sync::Arc;
use tokio::sync::Mutex;
use crate::astra_home::AstraHome;


pub struct HomeAssistantPlugin {

    home:
        AstraHome,

}





impl HomeAssistantPlugin {



    pub fn new(
        home: AstraHome
    ) -> Self {

        Self {
            home,
        }

    }

}

#[async_trait]
impl Plugin for HomeAssistantPlugin {



    fn name(
        &self
    ) -> &str {

        "home_assistant"

    }



    fn description(
        &self
    ) -> &str {

        "Home Assistant integration plugin"

    }
    

async fn start(
    &mut self
)
-> Result<()> {


    info(
        "ASTRA HOME START"
    );


    self.home
        .start()
        .await?;


    Ok(())

}

    async fn execute(
    &mut self,
    _command: String,
)
-> Result<PluginResponse> {


    Ok(
        PluginResponse {

            success: true,

            message:
                "Command handler moved to AstraHome"
                .to_string(),

        }
    )

}

}




pub fn create_plugin(
    config: Config
)
-> PluginInstance {


    let client =
        Arc::new(
            Mutex::new(
                HAClient::new(config)
            )
        );


    let home =
        AstraHome::new(
            client
        );


    let plugin =
        HomeAssistantPlugin::new(
            home
        );


    let manifest =
        PluginManifest::new(
            "homeassistant".to_string(),
            "Home Assistant Connector".to_string(),
            "0.1.4".to_string(),
            "Connect Astra to Home Assistant to read entity states, control devices, and call services.".to_string(),
        );


    let instance =
        PluginInstance::new(
            manifest,
            Box::new(plugin)
        );


    instance
}

