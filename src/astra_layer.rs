use crate::ha::client::HAClient;
use crate::core::response_generator::ResponseGenerator;
use crate::logger::info;

pub struct AstraLayer {

    ha_client: HAClient,

}


impl AstraLayer {


    pub fn new(
        ha_client: HAClient
    ) -> Self {

        Self {

            ha_client,

        }

    }



    pub async fn process(
        &mut self,
        command: &str
    ) -> String {


        info(
            &format!(
            "ASTRA COMMAND: {}",
            command
            )
        );


        let result =
            self.ha_client
                .process_command(command)
                .await;


        let response =
            ResponseGenerator::generate(
                &result
            );




        response

    }


}