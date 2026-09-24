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
#[derive(Debug)]
pub struct SearchResult<'a> {

    pub device: &'a DeviceState,

    pub score: u32,

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
pub enum ResolveResult<'a> {

    Found {
        device: &'a DeviceState,
        score: u32,
    },

    Multiple {
        devices: Vec<SearchResult<'a>>,
    },

    NotFound,

}