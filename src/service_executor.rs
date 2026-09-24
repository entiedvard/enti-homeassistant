use crate::core::models::{
    Action,
    DeviceState,
};


#[derive(Debug)]
pub struct ServiceCall {

    pub domain: String,

    pub service: String,

    pub entity_id: String,

}



pub struct ServiceExecutor;


impl ServiceExecutor {


    pub fn create_call(
        action: &Action,
        device: &DeviceState
    ) -> Option<ServiceCall> {


        match action {

    Action::TurnOn => {

        Some(ServiceCall {

            domain:
                device.domain.clone(),

            service:
                "turn_on".to_string(),

            entity_id:
                device.entity_id.clone(),

        })

    }


    Action::TurnOff => {

        Some(ServiceCall {

            domain:
                device.domain.clone(),

            service:
                "turn_off".to_string(),

            entity_id:
                device.entity_id.clone(),

        })

    }


    Action::Toggle => {

        Some(ServiceCall {

            domain:
                device.domain.clone(),

            service:
                "toggle".to_string(),

            entity_id:
                device.entity_id.clone(),

        })

    }


    _ => None,

}

    }

}