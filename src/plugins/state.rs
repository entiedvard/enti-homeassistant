#[derive(Debug, Clone)]
pub enum PluginState {

    Loaded,

    Starting,

    Running,

    Stopped,

    Error(String),

}
