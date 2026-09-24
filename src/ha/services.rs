use anyhow::Result;
use crate::ha::ws_ids::CALL_SERVICE_ID;
use futures_util::SinkExt;
use crate::service_executor::ServiceCall;
use crate::logger::{debug, error, info};
use crate::ha::client::HAClient;
use tokio_tungstenite::tungstenite::Message;

impl  HAClient {
    

pub async fn call_service(
    &mut self,

    call: ServiceCall,

) -> Result<()> {

debug(
    "ENTER CALL_SERVICE"
);



    let request =
        serde_json::json!({

            "id": CALL_SERVICE_ID,

            "type":
                "call_service",


            "domain":
                call.domain,


            "service":
                call.service,


            "target": {

                "entity_id":
                    call.entity_id

            }

        });

debug(
    &format!(
        "REQUEST: {}",
        request
    )
);

debug(
    &format!(
        "SERVICE CALL REQUEST: {}",
        request
    )
);


    // Отправка команды

    if let Some(sender) =
        &mut self.sender
    {
debug(
    "WEBSOCKET SENDER AVAILABLE"
);

        sender.send(

            Message::Text(
                request.to_string()
            )

        )
        .await?;


        info(
            "SERVICE SENT"
        );


    }
    else {

        error(
            "ERROR: WS sender unavailable"
        );

        return Ok(());

    }



    Ok(())

}
}