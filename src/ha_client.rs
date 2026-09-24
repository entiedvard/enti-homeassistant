use anyhow::Result;

use futures_util::{
    StreamExt,
    SinkExt,
};

use tokio_tungstenite::{
    connect_async,
    tungstenite::Message,
};

use serde_json::{
    json,
    Value,
};

use crate::models::{
    DeviceState,
    DomainFilter,
    ResolveResult,
};
use crate::entity_cache::EntityCache;



pub struct HAClient {

    pub url: String,

    pub token: String,

    pub cache: EntityCache,

}



impl HAClient {


    pub fn new(
        url: String,
        token: String,
    ) -> Self {

        Self {

            url,

            token,

            cache: EntityCache::new(),

        }

    }



    pub async fn connect(
        &mut self
    ) -> Result<()> {


        let (mut socket, _) =
            connect_async(&self.url)
            .await?;


        println!("Connected");



        // Получаем auth_required

        if let Some(message) =
            socket.next().await
        {

            println!(
                "{}",
                message?.into_text()?
            );

        }



        // Авторизация

        socket.send(

            Message::Text(

                json!({

                    "type":"auth",

                    "access_token":
                    self.token

                })
                .to_string()

            )

        )
        .await?;



        // Получаем auth_ok

        if let Some(message) =
            socket.next().await
        {

            println!(
                "{}",
                message?.into_text()?
            );

        }



        // Запрос всех состояний

        socket.send(

            Message::Text(

                json!(

                {

                    "id":1,

                    "type":"get_states"

                }

                )
                .to_string()

            )

        )
        .await?;



        // Получаем устройства

        if let Some(message) =
            socket.next().await
        {


            let text =
                message?.into_text()?;


            let data: Value =
                serde_json::from_str(&text)?;



            if let Some(list) =
                data["result"].as_array()
            {


                let mut entities:
                    Vec<DeviceState> =
                    Vec::new();



                for item in list {


                    let entity_id =
                    item["entity_id"]
                    .as_str()
                    .unwrap_or("")
                    .to_string();



                    let domain =
                    entity_id
                    .split('.')
                    .next()
                    .unwrap_or("unknown")
                    .to_string();



                    let friendly_name =
                    item["attributes"]
                    ["friendly_name"]
                    .as_str()
                    .unwrap_or(&entity_id)
                    .to_string();



                    let device =
                    DeviceState {

                    entity_id,

                    friendly_name,

                    domain,

                    state:
                    item["state"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),


                    last_changed:
                    item["last_changed"]
                    .as_str()
                    .unwrap_or("")
                    .to_string(),


                    attributes:
                    item["attributes"]
                    .clone(),

                };
                    self.cache.insert(
                        device.clone()
                    );
                    entities.push(device);

                }

            println!(
                "CACHE SIZE: {}",
                self.cache.count()
            );

                println!("\n=== DEVICES ===");



                let groups =
                [
                    "light",
                    "switch",
                    "sensor",
                    "automation",
                    "script",
                    "scene",
                ];



                for group in groups {


                    println!(
                        "\n-- {} --",
                        group
                    );



                    for device in
                    entities.iter()
                    .filter(
                        |d|
                        d.entity_type()
                        ==
                        group
                    )
                    {

                       println!(
                        "{} ({}) = {}",
                        device.friendly_name,
                        device.entity_id,
                        device.state
                        );

                    }

                }


            }

        }



        // Подписка на изменения

        socket.send(

            Message::Text(

                json!(

                {"id":2,

                    "type":
                    "subscribe_events",

                    "event_type":
                    "state_changed"

                }

                )
                .to_string()

            )

        )
        .await?;

        println!("\n=== SEARCH TEST ===");

        self.cache.print_all();


      println!("\n=== SMART SEARCH ===");





let results =
    self.cache.search(
        "свет",
        DomainFilter::Light
    );

for result in results {


    let level =
        EntityCache::confidence(
            result.score
        );


    println!(
        "{} ({}) => {}% {:?}",
        result.device.friendly_name,
        result.device.entity_id,
        result.score,
        level
    );

    println!("\n=== RESOLVE TEST ===");


let result =
    self.cache.resolve(
        "свет",
        DomainFilter::Light
    );


match result {


    ResolveResult::Found {
        device,
        score
    } => {

        println!(
            "FOUND: {} ({}) {}%",
            device.friendly_name,
            device.entity_id,
            score
        );

    }



    ResolveResult::Multiple {
        devices
    } => {

        println!("Multiple:");

        for d in devices {

            println!(
                "{} {}%",
                d.device.friendly_name,
                d.score
            );

        }

    }



    ResolveResult::NotFound => {

        println!("Nothing found");

    }

}

}
  



    

        println!(
            "\nListening events..."
        );



        // Постоянное ожидание событий

        while let Some(message) =
            socket.next().await
        {


            let text =
                message?.into_text()?;


            let data: Value =
                serde_json::from_str(&text)?;



            if data["type"]
                ==
                "event"
            {


                let event =
                    &data["event"]["data"];



                let entity =
                    event["entity_id"]
                    .as_str()
                    .unwrap_or("");



                let old =
                    event["old_state"]["state"]
                    .as_str()
                    .unwrap_or("");



                let new =
                    event["new_state"]["state"]
                    .as_str()
                    .unwrap_or("");



                println!();

                println!(
                    "CHANGE:"
                );


                if let Some(device) = self.cache.get(entity) {

    println!(
        "{} ({}) : {} -> {}",
        device.friendly_name,
        device.entity_id,
        old,
        new
    );

} else {

    println!(
        "{} : {} -> {}",
        entity,
        old,
        new
    );

}



                self.cache.update(

                    entity.to_string(),

                    new.to_string()

                );


            }


        }



        Ok(())

    }

}