use serde::{Serialize, Deserialize};
use serde_json::Value;


#[derive(Debug, Clone, Serialize, Deserialize)]
pub struct DeviceState {

    pub entity_id: String,

    pub friendly_name: String,

    pub domain: String,

    pub state: String,

    pub last_changed: String,

    pub attributes: Value,

}



impl DeviceState {


    pub fn entity_type(&self) -> String {

        self.domain.clone()

    }


}

impl Action {

    pub fn supports_domain(
        &self,
        domain: &str
    ) -> bool {


        match self {


            Action::TurnOn
            | Action::TurnOff
            | Action::Toggle => {

                matches!(
                    domain,
                    "light"
                    | "switch"
                    | "fan"
                    | "media_player"
                )

            }


            Action::SetBrightness(_) => {

                domain == "light"

            }


            Action::Unknown => {

                true

            }

        }

    }

}

#[derive(Debug, Clone)]
pub struct SearchResult {

    pub device: DeviceState,

    pub score: u32,

    pub reasons: Vec<ScoreReason>,

}

impl DomainFilter {

    pub fn matches(&self, domain: &str) -> bool {

        match self {

            DomainFilter::Any => true,

            DomainFilter::Light => domain == "light",

            DomainFilter::Switch => domain == "switch",

            DomainFilter::Sensor => domain == "sensor",

            DomainFilter::Automation => domain == "automation",

        }

    }

}

#[derive(Debug, Clone)]
pub enum DomainFilter {

    Any,

    Light,

    Switch,

    Sensor,

    Automation,

}
#[derive(Debug)]
pub enum SearchConfidence {

    High,

    Medium,

    Low,

}
#[derive(Debug)]
pub enum ResolveResult {

    Found {

        device: DeviceState,

        score: u32,

        reasons: Vec<ScoreReason>,

    },


    Multiple {

        devices: Vec<SearchResult>,

    },


    NotFound,

}

#[derive(Debug, Clone)]
pub struct ScoreReason {

    pub points: u32,

    pub reason: String,

}

#[derive(Debug, Clone)]
pub enum Action {

    TurnOn,

    TurnOff,

    Toggle,

    SetBrightness(u8),

    Unknown,

}

#[derive(Debug, Clone)]
pub struct CommandIntent {

    pub action: Action,

    pub target: String,

}

#[derive(Debug)]
pub enum CommandResult {

    Success {

        action: Action,

        device_name: String,

        entity_id: String,

        already_state: bool,

        confirmed_state: String,

    },


    DeviceNotFound {

        target: String,

    },


    MultipleDevices {

        count: usize,

    },


    ServiceError {

        error: String,

    },

    Timeout {
    device_name: String,
    entity_id: String,
    },

    ConnectionError {
        error: String,
    },

}