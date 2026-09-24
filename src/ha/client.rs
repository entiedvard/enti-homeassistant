use std::sync::Arc;

use tokio::sync::broadcast;

use crate::plugins::events::ConnectionStateChanged;

use crate::ha::connection;
use crate::ha::connection_state::ConnectionState;
use crate::config::Config;
use crate::ha::events::listen_events;
use crate::ha::command_state::{
    PendingCommand,
    ExpectedState,
};

use crate::ha::ws_ids::GET_STATES_ID;
use crate::ha::ws_ids::SUBSCRIBE_EVENTS_ID;
use crate::logger::{
    info,
    error,
    debug,
};

use anyhow::Result;

use futures_util::{
    StreamExt,
    SinkExt,
    stream::SplitStream,
};

use tokio::time::{timeout, Duration};

use tokio_tungstenite::tungstenite::protocol::frame::coding::Data::Continue;
use tokio_tungstenite::{
    tungstenite::Message,
    WebSocketStream,
    MaybeTlsStream,

};
use tokio::sync::Mutex;
use tokio::sync::oneshot;
use tokio::net::TcpStream;

use serde_json::{
    json,
    Value,
};

use crate::core::models::{
    Action, CommandResult, DeviceState, DomainFilter, ResolveResult,

};

use crate::core::entity_cache::EntityCache;
use crate::core::command_parser::CommandParser;
use crate::service_executor::ServiceExecutor;
use crate::core::command_executor::CommandExecutor;
use crate::service_executor::ServiceCall;
use tokio::sync::mpsc;



pub enum ConfirmationResult {

    AlreadySet,

    Changed {
        old_state: String,
        new_state: String,
    },

    Timeout,

}



pub struct HAClient {

    pub connection_state: ConnectionState,

    pub connection_state_sender:
    broadcast::Sender<ConnectionStateChanged>,

    connected: bool,

    config: Config,

    pub cache: Arc<Mutex<EntityCache>>,



pub sender:
    Option<mpsc::Sender<Message>>,

    pub pending_command:
    Arc<Mutex<Option<PendingCommand>>>,
}

use futures_util::{
    stream::SplitSink,
};


impl HAClient {
    pub fn subscribe_connection_state(
    &self
)
-> broadcast::Receiver<ConnectionStateChanged> {


    self.connection_state_sender
        .subscribe()

}

    pub fn is_connected(
    &self
) -> bool {

    self.connection_state ==
        ConnectionState::Connected

}

    pub fn connection_state(
    &self
) -> ConnectionState {

    self.connection_state.clone()

}
pub fn set_connection_state(
    &mut self,
    state: ConnectionState
) {


    let old_state =
        self.connection_state.clone();


    self.connection_state =
        state.clone();



    let _ =
        self.connection_state_sender.send(
            ConnectionStateChanged {

                old_state,

                new_state:
                    state,

            }
        );

}
pub async fn run(
    &mut self
) -> Result<()> {


    let mut reconnect_attempt: u32 = 0;


    loop {


        self.set_connection_state(
            ConnectionState::Connecting
        );


        info(
            "STARTING CONNECTION"
        );

self.connected = false;
self.sender = None;

        match self.connect().await {


            Ok(_) => {


                reconnect_attempt = 0;


                self.set_connection_state(
                    ConnectionState::Disconnected);


                info(
                    "HOME ASSISTANT CONNECTION CLOSED"
                );


            }



            Err(e) => {


                self.set_connection_state(
                    ConnectionState::Reconnecting);

                    self.pending_command
                    .lock()
                    .await
                    .take();
                
                error(
                    &format!(
                        "CONNECTION ERROR: {}",
                        e
                    )
                );


            }
Err(e) => {

    self.set_connection_state(
        ConnectionState::Error(
            e.to_string()
        )
    );


    info(
        &format!(
            "CONNECTION LOST: {}",
            e
        )
    );

}

        }

self.set_connection_state(
    ConnectionState::Reconnecting
);

        reconnect_attempt += 1;



        let delay =
            match reconnect_attempt {


                1 => 5,

                2 => 10,

                3 => 20,

                _ => 30,


            };



        info(
            &format!(
                "RECONNECT ATTEMPT: {}",
                reconnect_attempt
            )
        );


        info(
            &format!(
                "RECONNECT IN {} SECONDS",
                delay
            )
        );



        tokio::time::sleep(
            std::time::Duration::from_secs(
                delay
            )
        )
        .await;


    }

}



    pub async fn wait_for_state_change(
    &mut self,
    entity_id: String,
) -> Result<String> {

debug(
    "WAITING FOR STATE EVENT"
);

let (tx, rx) =
    oneshot::channel();


   let mut pending =
    self.pending_command.lock().await;

*pending = Some(
    PendingCommand {
        entity_id: entity_id.clone(),
        sender: tx,
        expected_state: ExpectedState::On, // Здесь можно указать ожидаемое состояние
    }
);

    let state =
        rx.await?;


    Ok(state)

}



pub async fn process_command(
    &mut self,
    command: &str,
) -> CommandResult {




    let intent =
        CommandParser::parse(
            command
        );





    let call =
    {
        let cache_guard =
            self.cache.lock().await;

if !self.is_connected() {

    return CommandResult::ConnectionError{
        error:
        "Home Assistant недоступен".to_string()
    };

}
        CommandExecutor::resolve_command(
            &intent,
            &cache_guard,
            
        )
    };


    if let Some(call) = call {


        let device_name =
        {
            let cache_guard =
                self.cache.lock().await;


            cache_guard
                .get(&call.entity_id)
                .map(
                    |d| d.friendly_name.clone()
                )
                .unwrap_or(
                    call.entity_id.clone()
                )
        };


        let entity_id =
            call.entity_id.clone();


        let action =
            intent.action.clone();
let expected_state =
    match &action {

        Action::TurnOn =>
            ExpectedState::On,

        Action::TurnOff =>
            ExpectedState::Off,

        _ =>
            ExpectedState::On,

    };

                let current_state =
    {
        let cache_guard =
            self.cache.lock().await;


        cache_guard
            .get(&entity_id)
            .map(|d| d.state.clone())
    };

            if let Some(state) = current_state {

            let already_correct =
    match &expected_state {

        ExpectedState::On =>
            state == "on",

        ExpectedState::Off =>
            state == "off",

    };


if already_correct {




                return CommandResult::Success {

                    action,

                    device_name,


                    entity_id,

                    already_state: true,

                    confirmed_state: state,

                };

            }

        }
        
        

        // Создаём канал ожидания подтверждения

        let (tx, rx) =
            oneshot::channel();



        // Регистрируем ожидаемую команду

        {
            let mut pending =
                self.pending_command.lock().await;


            *pending =
                Some(
                    PendingCommand {

                        entity_id:
                            entity_id.clone(),

                        sender:
                            tx,
                        expected_state,
                    }
                );
        }



        // Отправляем команду в Home Assistant

        match self.call_service(call).await {


            Ok(_) => {

info(
    "SERVICE SENT"
);


debug(
    "WAITING FOR STATE CONFIRMATION"
);



                // Ждём подтверждение с таймаутом

                let result =
                    timeout(
                       Duration::from_secs(
    self.config.command.confirmation_timeout
),
                        rx
                    )
                    .await;



                match result {


                    // Таймаут не сработал,
                    // канал получил ответ

                    Ok(received) => {


                        match received {


                            Ok(state) => {


error(
    "STATE CONFIRMATION TIMEOUT"
);

                                return CommandResult::Success {

                                    action,

                                    device_name,

                                    entity_id,

                                    already_state: false,

                                    confirmed_state: state,

                                };


                            }


                            Err(e) => {


                                return CommandResult::ServiceError {

                                    error:
                                        e.to_string(),

                                };


                            }

                        }

                    }



                    // 5 секунд прошло

                    Err(_) => {


                        error(
                            "STATE CONFIRMATION TIMEOUT"
                        );

                        let mut pending =
                            self.pending_command.lock().await;

                        *pending = None;

                        return CommandResult::Timeout {

                            device_name,

                            entity_id,

                        };

                    }

                }

            }



            Err(e) => {


                return CommandResult::ServiceError {

                    error:
                        e.to_string(),

                };


            }

        }

    }



    CommandResult::DeviceNotFound {

        target:
            intent.target,

    }

}
    pub fn new(
    config: Config
) -> Self {
            let (connection_state_sender, _) =
    broadcast::channel(32);

    Self {
        connection_state_sender,



        connection_state:
            ConnectionState::Disconnected,

        connected: false,

        config,

        cache: Arc::new(
            Mutex::new(
                EntityCache::new()
            )
        ),

        sender: None,

        pending_command:
            Arc::new(
                Mutex::new(None)
            ),

    }

}
async fn load_states(
    &mut self,

    write: &mut SplitSink<
        WebSocketStream<MaybeTlsStream<TcpStream>>,
        Message
    >,

    read: &mut SplitStream<
        WebSocketStream<MaybeTlsStream<TcpStream>>
    >,

) -> Result<()> {


    let request =
        serde_json::json!({

            "id": GET_STATES_ID,

            "type": "get_states"

        });


info(
    "REQUESTING HOME ASSISTANT STATES"
);


    write.send(
        Message::Text(
            request.to_string()
        )
    )
    .await?;



    if let Some(message) =
        read.next().await
    {


        let text =
            message?.into_text()?;


        let data: Value =
            serde_json::from_str(&text)?;



        if let Some(list) =
            data["result"].as_array()
        {


info(
    &format!(
        "DEVICES RECEIVED: {}",
        list.len()
    )
);


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



                let mut cache =
                self.cache.lock().await;


            cache.insert(device);

            }


            let cache =
            self.cache.lock().await;


debug(
    &format!(
        "CACHE SIZE: {}",
        cache.len()
    )
);

        }

    }


    Ok(())

}
pub async fn send_message(
    &mut self,
    message: Message
) {

        if let Some(sender) = &mut self.sender {

            sender.send(message)
                .await
                .unwrap();

        }
        else {



        }

    }
    //авторзация 
    async fn authenticate(
    &self,

    write: &mut SplitSink<
        WebSocketStream<MaybeTlsStream<TcpStream>>,
        Message
    >,

    read: &mut SplitStream<
        WebSocketStream<MaybeTlsStream<TcpStream>>
    >,

) -> Result<()> {


    // Получаем auth_required

    if let Some(message) =
        read.next().await
    {



    }


    // Отправляем токен

    write.send(

        Message::Text(

            serde_json::json!({

                "type":"auth",

                "access_token":
                    self.config.homeassistant.token

            })
            .to_string()

        )

    )
    .await?;



    // Получаем auth_ok

    if let Some(message) =
        read.next().await
    {


    }


    Ok(())

}
async fn subscribe_events(
    &self,

    write: &mut SplitSink<
        WebSocketStream<MaybeTlsStream<TcpStream>>,
        Message
    >,

) -> Result<()> {

    let request =
        serde_json::json!({

            "id": SUBSCRIBE_EVENTS_ID,

            "type":
                "subscribe_events",

            "event_type":
                "state_changed"

        });


    write.send(
        Message::Text(
            request.to_string()
        )
    )
    .await?;


 


    Ok(())

}

pub async fn connect(
    &mut self,
) -> Result<()> {

    self.sender = None;
    self.connected = false;


    let (disconnect_tx, mut disconnect_rx) =
        tokio::sync::mpsc::channel::<Result<()>>(10);


    if self.connected {
        return Ok(());
    }


    self.set_connection_state(
        ConnectionState::Connecting
    );


    let socket =
        match connection::open_connection(
            &self.config.homeassistant.url
        )
        .await
    {

        Ok(socket) => socket,


        Err(e) => {

            self.set_connection_state(
                ConnectionState::Error(
                    e.to_string()
                )
            );


            return Err(e.into());

        }

    };


    let (mut write, mut read) =
        socket.split();



    connection::authenticate(
        &mut write,
        &mut read,
        &self.config.homeassistant.token
    )
    .await?;



    self.load_states(
        &mut write,
        &mut read
    )
    .await?;



    connection::subscribe_events(
        &mut write
    )
    .await?;



    self.set_connection_state(
        ConnectionState::Connected
    );



    let cache =
        self.cache.clone();


    let pending_command =
        self.pending_command.clone();



   let (tx, mut rx) =
    mpsc::channel::<Message>(64);

tokio::spawn(async move {

    while let Some(message) =
        rx.recv().await
    {

        if let Err(e) =
            write.send(message).await
        {

            error(
                &format!(
                    "WEBSOCKET WRITE ERROR: {}",
                    e
                )
            );


            let _ =
                disconnect_tx.send(
                    Err(e.into())
                );


            break;
        }

    }


    info(
        "WRITER TASK STOPPED"
    );

});

info(
    "HEARTBEAT STARTED"
);
let heartbeat_sender =
    tx.clone();


tokio::spawn(async move {

    let mut timer =
        tokio::time::interval(
            Duration::from_secs(30)
        );


    loop {

        timer.tick().await;

debug(
    "HEARTBEAT PING"
);
        if heartbeat_sender
            .send(
                Message::Ping(vec![])
            )
            .await
            .is_err()
        {

            break;

        }

    }

});

    self.sender = Some(tx);


    self.connected = true;



    info(
        "CONNECT FUNCTION FINISHED"
    );



    // Ждём завершения listener.
    // Пока websocket жив - функция здесь стоит.

    match disconnect_rx.recv().await {

    Some(Ok(_)) => {

        anyhow::bail!(
            "CONNECTION CLOSED"
        );

    }


    Some(Err(e)) => {

        anyhow::bail!(
            "CONNECTION ERROR: {}",
            e
        );

    }


    None => {

        anyhow::bail!(
            "DISCONNECT CHANNEL CLOSED"
        );

    }

}
match disconnect_rx.recv().await {

    Some(Ok(_)) => {

        self.connected = false;

        anyhow::bail!(
            "CONNECTION CLOSED"
        );

    }


    Some(Err(e)) => {

        self.connected = false;

        anyhow::bail!(
            "CONNECTION ERROR: {}",
            e
        );

    }


    None => {

        self.connected = false;

        anyhow::bail!(
            "DISCONNECT CHANNEL CLOSED"
        );

    }

}
}

}

