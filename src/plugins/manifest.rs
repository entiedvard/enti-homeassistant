use serde::Deserialize;


#[derive(Debug, Deserialize, Clone)]
pub struct PluginManifest {

    pub plugin: PluginInfo,

    pub capabilities: Option<PluginCapabilities>,

    pub dependencies: Option<PluginDependencies>,

    pub runtime: Option<PluginRuntime>,

    pub permissions: Option<PluginPermissions>,

}


#[derive(Debug, Deserialize, Clone)]
pub struct PluginInfo {

    pub id: String,

    pub name: String,

    pub version: String,

    pub description: String,

}


#[derive(Debug, Deserialize, Clone)]
pub struct PluginCapabilities {

    pub events: bool,

    pub commands: bool,

    pub configuration: bool,

}


#[derive(Debug, Deserialize, Clone)]
pub struct PluginDependencies {

    pub core: Option<String>,

}


#[derive(Debug, Deserialize, Clone)]
pub struct PluginRuntime {

    pub auto_start: bool,

    pub restart: bool,

}


#[derive(Debug, Deserialize, Clone)]
pub struct PluginPermissions {

    pub homeassistant_read: bool,

    pub homeassistant_control: bool,

}

impl PluginManifest {

    pub fn new(
        id: String,
        name: String,
        version: String,
        description: String,
    ) -> Self {

        Self {

            plugin: PluginInfo {

                id,

                name,

                version,

                description,

            },

            capabilities: None,

            dependencies: None,

            runtime: None,

            permissions: None,

        }

    }

}