use crate::{
    core::entity_cache::EntityCache,
    core::models::{
        CommandIntent,
        ResolveResult,
        DomainFilter,
    },
    service_executor::ServiceExecutor,
    service_executor::ServiceCall,
};


use crate::logger::{
    info,
    debug,
    error,
};
pub struct CommandExecutor;


impl CommandExecutor {



    pub fn resolve_command(
        intent: &CommandIntent,
        cache: &EntityCache,
    ) -> Option<ServiceCall> {


        let result =
            cache.resolve(
                &intent.target,
                DomainFilter::Light
            );


        match result {


            ResolveResult::Found {
                device,
                ..
            } => {


                ServiceExecutor::create_call(
                    &intent.action,
                    &device
                )


            }


            ResolveResult::Multiple {
                devices
            } => {

                info("Multiple devices found: {}");

                None

            }


            ResolveResult::NotFound => {

                debug(
                    &format!(
                    "Device not found"
                    )
                );

                None

            }

        }

    }

}