use anyhow::Result;

use std::collections::HashMap;

use tokio::sync::broadcast;

use crate::plugins::events::PluginStateChanged;
use crate::plugins::instance::PluginInstance;
use crate::plugins::state::PluginState;
use crate::info;

pub struct PluginManager {


    plugins:
        HashMap<String, PluginInstance>,


    pub state_sender:
        broadcast::Sender<PluginStateChanged>,


}

impl PluginManager {



pub fn subscribe(
    &self
)
-> broadcast::Receiver<PluginStateChanged> {


    self.state_sender.subscribe()

}
pub fn register(
    &mut self,
    plugin: PluginInstance
) {


    let id =
        plugin
        .manifest
        .plugin
        .id
        .clone();


    self.plugins.insert(
        id,
        plugin
    );


}
pub fn get(
    &self,
    id: &str
)
-> Option<&PluginInstance> {


    self.plugins.get(id)

}
pub fn list(
    &self
)
-> Vec<&PluginInstance> {


    self.plugins
        .values()
        .collect()


}


pub async fn start(
    &mut self,
    id: &str
) -> Result<()> {


    let plugin =
        self.plugins
        .get_mut(id)
        .ok_or_else(|| {
            anyhow::anyhow!(
                "Plugin not found: {}",
                id
            )
        })?;


    plugin.set_state(
        PluginState::Starting,
        &self.state_sender
    );


    match plugin.start().await {


        Ok(_) => {

            plugin.set_state(
                PluginState::Running,
                &self.state_sender
            );

        }


        Err(e) => {

            plugin.set_state(
                PluginState::Error(
                    e.to_string()
                ),
                &self.state_sender
            );


            return Err(e);

        }

    }


    Ok(())

} 
pub async fn start_all(
    &mut self
) -> Result<()> {

    let plugins: Vec<String> =
        self.plugins
            .keys()
            .cloned()
            .collect();


for id in plugins {

    self.start(
        &id
    )
    .await?;

}


    Ok(())

}

    pub fn new() -> Self {

        let (state_sender, _) =
            broadcast::channel(32);


        Self {

            plugins:
                HashMap::new(),

            state_sender,

        }

    }
}