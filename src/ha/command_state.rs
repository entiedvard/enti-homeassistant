use tokio::sync::oneshot;

#[derive(Clone, Debug)]
pub enum ExpectedState {

    On,

    Off,

}

pub struct PendingCommand {
    pub entity_id: String,
    pub sender: oneshot::Sender<String>,

    pub expected_state:
        ExpectedState,

}