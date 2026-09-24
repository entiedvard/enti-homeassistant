use anyhow::Result;

use crate::ha::ws_ids::SUBSCRIBE_EVENTS_ID;
use futures_util::{
    SinkExt,
    StreamExt,
    stream::{
        SplitSink,
        SplitStream,
    },
};

use serde_json::json;
use tokio::time::{
    timeout,
    Duration,
    interval,
};
use tokio::net::TcpStream;

use tokio_tungstenite::{
    connect_async,
    MaybeTlsStream,
    WebSocketStream,
    tungstenite::Message,
};

use crate::logger::{
    debug,
    error,
    info,
};


// Типы половинок WebSocket

pub type WriteHalf =
    SplitSink<
        WebSocketStream<MaybeTlsStream<TcpStream>>,
        Message
    >;


pub type ReadHalf =
    SplitStream<
        WebSocketStream<MaybeTlsStream<TcpStream>>
    >;



// Создание WebSocket соединения

pub async fn open_connection(
    url: &str,
) -> Result<WebSocketStream<MaybeTlsStream<TcpStream>>> {


    info(
        "CONNECTING TO HOME ASSISTANT"
    );


let connection =
    timeout(
        Duration::from_secs(10),
        connect_async(url)
    )
    .await;


let (socket, _) =
    match connection {

        Ok(result) => result?,

        Err(_) => {

            anyhow::bail!(
                "Home Assistant connection timeout"
            );

        }

    };


    info(
        "WEBSOCKET CONNECTED"
    );


    Ok(socket)

}

pub async fn heartbeat(
    write: &mut WriteHalf
) -> Result<()> {


    let mut timer =
        interval(
            Duration::from_secs(30)
        );


    loop {

        timer.tick().await;


        write.send(
            Message::Ping(
                vec![]
            )
        )
        .await?;

    }

}

// Авторизация Home Assistant

pub async fn authenticate(
    write: &mut WriteHalf,
    read: &mut ReadHalf,
    token: &str,
) -> Result<()> {


    let auth_result =
        timeout(
            Duration::from_secs(10),
            async {

                while let Some(message) =
                    read.next().await
                {


                    let message =
                        message?;


                    let text =
                        message.to_text()?;


                    debug(
                        &format!(
                            "AUTH EVENT: {}",
                            text
                        )
                    );


                    let value: serde_json::Value =
                        serde_json::from_str(text)?;



                    match value["type"]
                        .as_str()
                    {


                        Some("auth_required") =>
                        {


                            let auth =
                                json!(
                                {
                                    "type": "auth",
                                    "access_token": token
                                });


                            write.send(
                                Message::Text(
                                    auth.to_string()
                                )
                            )
                            .await?;


                        }



                        Some("auth_ok") =>
                        {

                            info(
                                "AUTH OK"
                            );


                            return Ok::<(), anyhow::Error>(());

                        }



                        Some("auth_invalid") =>
                        {

                            error(
                                "AUTH FAILED"
                            );


                            anyhow::bail!(
                                "Home Assistant authentication failed"
                            );

                        }



                        _ => {}

                    }

                }


                anyhow::bail!(
                    "Connection closed during authentication"
                );


            }
        )
        .await;



    match auth_result {

        Ok(result) =>
            result,


        Err(_) => {

            error(
                "AUTH TIMEOUT"
            );


            anyhow::bail!(
                "Home Assistant authentication timeout"
            );

        }

    }

}


// Подписка на события HA

pub async fn subscribe_events(
    write: &mut WriteHalf,
) -> Result<()> {


    let request =
        json!(
        {
            "id": SUBSCRIBE_EVENTS_ID,
            "type": "subscribe_events"
        });



    write.send(
        Message::Text(
            request.to_string()
        )
    )
    .await?;


    info(
        "EVENT SUBSCRIPTION SENT"
    );


    Ok(())

}