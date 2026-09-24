use futures_util::StreamExt;
use serde_json::Value;
use anyhow::Result;
use std::time::Duration;
use crate::logger::{
    debug,
    info,
};


use tokio::sync::Mutex;
use std::sync::Arc;

use tokio_tungstenite::{
    WebSocketStream,
    MaybeTlsStream,
};

use tokio::net::TcpStream;

use futures_util::stream::SplitStream;

use crate::core::models::{
    DeviceState,
};
use crate::core::entity_cache::EntityCache;
use crate::ha::command_state::{
    PendingCommand,
    ExpectedState,
};


pub async fn listen_events(
    mut read: SplitStream<WebSocketStream<MaybeTlsStream<TcpStream>>>,
    cache: Arc<Mutex<EntityCache>>,
    pending_command: Arc<Mutex<Option<PendingCommand>>>
) -> Result<()> {


info(
    "EVENT LISTENER STARTED"
);


while let Some(message) =
    read.next().await
{

    let message =
        message?;


    let text =
        message.into_text()?;


    if text.trim().is_empty() {
        continue;
    }


    let data: Value =
        serde_json::from_str(&text)?;



 

  


  


        if text.trim().is_empty() {
            continue;
        }


        let data: Value =
            serde_json::from_str(&text)?;



        if data["type"] == "event"
        {


            let event =
                &data["event"]["data"];


            let entity =
                event["entity_id"]
                .as_str()
                .unwrap_or("")
                .to_string();
            if entity.is_empty() {
    continue;
}


            let old =
                event["old_state"]["state"]
                .as_str()
                .unwrap_or("")
                .to_string();


            let new =
                event["new_state"]["state"]
                .as_str()
                .unwrap_or("")
                .to_string();

            if entity.is_empty()
                || new.is_empty()
            {
                continue;
            }

debug(
    &format!(
        "ENTITY EVENT: {}",
        entity
    )
);


debug(
    &format!(
        "OLD: {}\nNEW: {}",
        old,
        new
    )
);





            // Вывод имени устройства из кеша

            {
                let cache_guard =
                    cache.lock().await;


                if let Some(device) =
                    cache_guard.get(&entity)
                {

debug(
    &format!(
        "DEVICE: {}",
        device.friendly_name
    )
);

                }
                else
                {

debug(
    &format!(
        "STATE CHANGE: {} {} -> {}",
        entity,
        old,
        new
    )
);

                }

            }



            // Проверяем ожидаемую команду

            {
                
                let mut pending =
                    pending_command.lock().await;


                if let Some(command) =
                    pending.as_ref()
                {

debug(
    &format!(
        "PENDING CHECK: {} EXPECTED {:?}",
        command.entity_id,
        command.expected_state
    )
);

                    if command.entity_id == entity
                    {
debug(
    &format!(
        "ENTITY MATCH CHECK: {} == {}",
        command.entity_id,
        entity
    )
);

                    let state_matches =
                        match &command.expected_state {

                            ExpectedState::On =>
                                new == "on",

                            ExpectedState::Off =>
                                new == "off",

                        };

debug(
    &format!(
        "STATE MATCH CHECK: expected={:?} actual={}",
        command.expected_state,
        new
    )
);
                    if state_matches {

                            info(
                                "COMMAND CONFIRMED"
                            );


                            let command =
                                pending.take();


                            if let Some(command) =
                                command
                            {

                                let _ =
                                    command.sender.send(
                                        new.clone()
                                    );

                            }


                        }
                        else
                        {

debug(
    &format!(
        "STATE CHANGED BUT NOT EXPECTED: {}",
        new
    )
);
                        }

                    }

                }

            }



            // Обновляем кеш

            {
                let mut cache_guard =
                    cache.lock().await;


                cache_guard.update(
                    entity.clone(),
                    new.clone()
                );

            }


debug(
&format!(
    "FULL EVENT:\n{}",
    text
    )
);


        }

    }


   info(
    "EVENT STREAM CLOSED"
);

anyhow::bail!(
    "EVENT STREAM CLOSED"
);

}
