use crate::plugins::state::PluginState;
use crate::ha::connection_state::ConnectionState;

#[derive(Debug, Clone)]
pub struct PluginStateChanged {


    pub plugin_name: String,


    pub old_state: PluginState,


    pub new_state: PluginState,


}
#[derive(Debug, Clone)]
pub struct ConnectionStateChanged {

    pub old_state: ConnectionState,

    pub new_state: ConnectionState,

}