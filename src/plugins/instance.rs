use anyhow::Result;

use tokio::sync::broadcast::Sender;

use crate::plugins::manifest::PluginManifest;
use crate::plugins::state::PluginState;
use crate::plugins::events::PluginStateChanged;


use crate::plugins::interface::Plugin;


pub struct PluginInstance {


    pub manifest: PluginManifest,


    pub state: PluginState,


    pub running: bool,


    pub plugin:
        Box<dyn Plugin>,


}

impl PluginInstance {


pub fn id(
    &self
) -> &str {

    &self.manifest.plugin.id

}



pub fn name(
    &self
) -> &str {

    &self.manifest.plugin.name

}



pub fn version(
    &self
) -> &str {

    &self.manifest.plugin.version

}

pub fn has_commands(
    &self
) -> bool {


    self.manifest
        .capabilities
        .as_ref()
        .map(
            |c| c.commands
        )
        .unwrap_or(false)

}
pub async fn start(
    &mut self,
)
-> Result<()> {


    self.plugin.start().await?;


    self.running = true;


    Ok(())

}

pub fn set_state(
    &mut self,
    new_state: PluginState,
    sender: &Sender<PluginStateChanged>,
) {


    let old_state =
        self.state.clone();


    self.state =
        new_state.clone();


    let _ =
        sender.send(
            PluginStateChanged {

                plugin_name:
                    self.manifest
                    .plugin
                    .name
                    .clone(),

                old_state,

                new_state,

            }
        );


}
pub fn new(
    manifest: PluginManifest,
    plugin: Box<dyn Plugin>,
)
-> Self {


    Self {

        manifest,

        state:
            PluginState::Loaded,

        running:
            false,

        plugin,

    }


}
}