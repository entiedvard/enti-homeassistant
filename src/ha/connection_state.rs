#[derive(
    Debug,
    Clone,
    PartialEq,
    Eq
)]
pub enum ConnectionState {

    Disconnected,

    Connecting,

    Connected,

    Reconnecting,

    Error(String),

}