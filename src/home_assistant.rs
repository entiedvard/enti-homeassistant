use astra_plugin_sdk::ToolError;
use astra_plugin_sdk::prelude::*;
use futures_util::{SinkExt, StreamExt};
use std::{collections::VecDeque, sync::Arc, time::Duration};
use tokio::sync::Mutex;
use tokio_tungstenite::{connect_async, tungstenite::Message};

const REQUEST_TIMEOUT_SECONDS: u64 = 20;
const MAX_STATE_CHANGES: usize = 100;

pub type StateChangeCache = Arc<Mutex<VecDeque<JsonValue>>>;

pub fn state_change_cache() -> StateChangeCache {
    Arc::new(Mutex::new(VecDeque::with_capacity(MAX_STATE_CHANGES)))
}

pub struct HomeAssistantClient {
    url: String,
    access_token: String,
}
pub struct ServiceCall<'a> {
    domain: &'a str,
    service: &'a str,
    entity_id: &'a str,
    data: JsonValue,
}
pub struct Entity {
    value: JsonValue,
}

impl HomeAssistantClient {
    pub fn new(url: String, access_token: String) -> Self {
        Self { url, access_token }
    }
    pub async fn states(&self) -> Result<Vec<Entity>, ToolError> {
        tokio::time::timeout(
            Duration::from_secs(REQUEST_TIMEOUT_SECONDS),
            self.states_inner(),
        )
        .await
        .map_err(|_| {
            ToolError::Timeout("Истекло общее время выполнения команды Home Assistant".into())
        })?
    }
    async fn states_inner(&self) -> Result<Vec<Entity>, ToolError> {
        let result = self
            .connect()
            .await?
            .request(json!({ "type": "get_states" }))
            .await?;
        let states = result.as_array().ok_or_else(|| {
            ToolError::Internal("Home Assistant вернул некорректный ответ get_states".into())
        })?;
        Ok(states
            .iter()
            .cloned()
            .map(|value| Entity { value })
            .collect())
    }
    pub async fn state(&self, entity_id: &str) -> Result<Entity, ToolError> {
        self.states()
            .await?
            .into_iter()
            .find(|state| state.value["entity_id"].as_str() == Some(entity_id))
            .ok_or_else(|| ToolError::NotFound(format!("Сущность {entity_id} не найдена")))
    }
    pub async fn call_service(&self, call: ServiceCall<'_>) -> Result<(), ToolError> {
        tokio::time::timeout(
            Duration::from_secs(REQUEST_TIMEOUT_SECONDS),
            self.call_service_inner(call),
        )
        .await
        .map_err(|_| {
            ToolError::Timeout("Истекло общее время выполнения команды Home Assistant".into())
        })?
    }
    async fn call_service_inner(&self, call: ServiceCall<'_>) -> Result<(), ToolError> {
        let mut request = json!({ "type": "call_service", "domain": call.domain, "service": call.service, "service_data": call.data });
        if !call.entity_id.is_empty() {
            request["target"] = json!({ "entity_id": call.entity_id });
        }
        self.connect().await?.request(request).await.map(|_| ())
    }
    pub async fn listen_state_changes(&self, cache: StateChangeCache) -> Result<(), ToolError> {
        let mut connection = self.connect().await?;
        connection
            .request(json!({
                "type": "subscribe_events",
                "event_type": "state_changed"
            }))
            .await?;
        loop {
            let message = connection.next_json_wait().await?;
            if message["type"].as_str() != Some("event")
                || message["event"]["event_type"].as_str() != Some("state_changed")
            {
                continue;
            }
            let mut events = cache.lock().await;
            if events.len() == MAX_STATE_CHANGES {
                events.pop_front();
            }
            events.push_back(message["event"].clone());
        }
    }
    async fn connect(&self) -> Result<Connection, ToolError> {
        let (socket, _) = tokio::time::timeout(
            std::time::Duration::from_secs(REQUEST_TIMEOUT_SECONDS),
            connect_async(&self.url),
        )
        .await
        .map_err(|_| ToolError::Timeout("Истекло время подключения к Home Assistant".into()))?
        .map_err(|error| {
            ToolError::Unavailable(format!("Не удалось подключиться к Home Assistant: {error}"))
        })?;
        let mut connection = Connection { socket, next_id: 1 };
        connection.authenticate(&self.access_token).await?;
        Ok(connection)
    }
}

impl<'a> ServiceCall<'a> {
    pub fn new(domain: &'a str, service: &'a str, entity_id: &'a str, data: JsonValue) -> Self {
        Self {
            domain,
            service,
            entity_id,
            data,
        }
    }
    pub fn for_entity(
        domain: &'a str,
        service: &'a str,
        entity_id: &'a str,
        data: JsonValue,
    ) -> Self {
        Self::new(domain, service, entity_id, data)
    }
}

impl Entity {
    pub fn domain(&self) -> String {
        self.value["entity_id"]
            .as_str()
            .and_then(|id| id.split_once('.').map(|(domain, _)| domain))
            .unwrap_or_default()
            .to_lowercase()
    }
    pub fn matches(&self, query: &str) -> bool {
        let entity_id = self.value["entity_id"]
            .as_str()
            .unwrap_or_default()
            .to_lowercase();
        let friendly_name = self.value["attributes"]["friendly_name"]
            .as_str()
            .unwrap_or_default()
            .to_lowercase();
        entity_id.contains(query) || friendly_name.contains(query)
    }
    pub fn summary(&self) -> JsonValue {
        json!({ "entity_id": self.value["entity_id"], "friendly_name": self.value["attributes"]["friendly_name"], "state": self.value["state"] })
    }
    pub fn full(&self) -> JsonValue {
        self.value.clone()
    }
}

struct Connection {
    socket: tokio_tungstenite::WebSocketStream<
        tokio_tungstenite::MaybeTlsStream<tokio::net::TcpStream>,
    >,
    next_id: u64,
}

impl Connection {
    async fn authenticate(&mut self, access_token: &str) -> Result<(), ToolError> {
        tokio::time::timeout(
            std::time::Duration::from_secs(REQUEST_TIMEOUT_SECONDS),
            self.authenticate_inner(access_token),
        )
        .await
        .map_err(|_| ToolError::Timeout("Истекло время авторизации Home Assistant".into()))?
    }
    async fn authenticate_inner(&mut self, access_token: &str) -> Result<(), ToolError> {
        let required = self.next_json().await?;
        match required["type"].as_str() {
            Some("auth_required") => {}
            Some("auth_invalid") => return Err(authentication_error(&required)),
            Some(kind) => {
                return Err(ToolError::Unavailable(format!(
                    "Home Assistant прислал `{kind}` вместо auth_required"
                )));
            }
            None => {
                return Err(ToolError::Internal(
                    "Home Assistant прислал сообщение без поля type при авторизации".into(),
                ));
            }
        }
        self.send(json!({ "type": "auth", "access_token": access_token }))
            .await?;
        let response = self.next_json().await?;
        match response["type"].as_str() {
            Some("auth_ok") => Ok(()),
            Some("auth_invalid") => Err(authentication_error(&response)),
            Some(kind) => Err(ToolError::Unavailable(format!(
                "Home Assistant прислал `{kind}` вместо auth_ok"
            ))),
            None => Err(ToolError::Internal(
                "Home Assistant прислал сообщение без поля type при авторизации".into(),
            )),
        }
    }
    async fn request(&mut self, request: JsonValue) -> Result<JsonValue, ToolError> {
        tokio::time::timeout(
            std::time::Duration::from_secs(REQUEST_TIMEOUT_SECONDS),
            self.request_inner(request),
        )
        .await
        .map_err(|_| {
            ToolError::Timeout("Истекло общее время выполнения команды Home Assistant".into())
        })?
    }
    async fn request_inner(&mut self, mut request: JsonValue) -> Result<JsonValue, ToolError> {
        let id = self.next_id;
        self.next_id += 1;
        request["id"] = json!(id);
        self.send(request).await?;
        loop {
            let response = self.next_json().await?;
            if response["type"].as_str() == Some("result") && response["id"].as_u64() == Some(id) {
                if response["success"].as_bool() == Some(true) {
                    return Ok(response["result"].clone());
                }
                return Err(service_error(&response));
            }
        }
    }
    async fn send(&mut self, value: JsonValue) -> Result<(), ToolError> {
        self.socket
            .send(Message::Text(value.to_string().into()))
            .await
            .map_err(network_error)
    }
    async fn next_json(&mut self) -> Result<JsonValue, ToolError> {
        tokio::time::timeout(
            std::time::Duration::from_secs(REQUEST_TIMEOUT_SECONDS),
            self.next_json_wait(),
        )
        .await
        .map_err(|_| ToolError::Timeout("Истекло время ожидания ответа Home Assistant".into()))?
    }
    async fn next_json_wait(&mut self) -> Result<JsonValue, ToolError> {
        loop {
            let message = self
                .socket
                .next()
                .await
                .ok_or_else(|| {
                    ToolError::Unavailable("Home Assistant закрыл WebSocket-соединение".into())
                })?
                .map_err(network_error)?;
            match message {
                Message::Text(text) => {
                    return serde_json::from_str(&text).map_err(|_| {
                        ToolError::Internal("Home Assistant прислал некорректный JSON".into())
                    });
                }
                Message::Ping(data) => self
                    .socket
                    .send(Message::Pong(data))
                    .await
                    .map_err(network_error)?,
                Message::Close(_) => {
                    return Err(ToolError::Unavailable(
                        "Home Assistant закрыл WebSocket-соединение".into(),
                    ));
                }
                _ => {}
            }
        }
    }
}

fn network_error(error: tokio_tungstenite::tungstenite::Error) -> ToolError {
    ToolError::Unavailable(format!("Ошибка WebSocket Home Assistant: {error}"))
}

fn authentication_error(response: &JsonValue) -> ToolError {
    let message = response["message"]
        .as_str()
        .filter(|message| !message.trim().is_empty())
        .map(|message| format!(": {message}"))
        .unwrap_or_default();
    ToolError::Unauthorized(format!("Home Assistant отклонил access token{message}"))
}

fn service_error(response: &JsonValue) -> ToolError {
    let code = response["error"]["code"]
        .as_str()
        .unwrap_or("unknown_error");
    let message = response["error"]["message"]
        .as_str()
        .unwrap_or("Home Assistant не вернул описание ошибки")
        .replace(['\r', '\n'], " ");
    ToolError::Internal(format!(
        "Home Assistant отклонил запрос ({code}): {message}"
    ))
}

#[cfg(test)]
mod tests {
    use super::*;
    use tokio::net::TcpListener;
    use tokio_tungstenite::accept_async;

    #[tokio::test]
    async fn authenticates_and_gets_states_from_home_assistant() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock server");
        let address = listener.local_addr().expect("mock server address");
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept client");
            let mut socket = accept_async(stream).await.expect("upgrade websocket");
            socket
                .send(Message::Text(
                    json!({ "type": "auth_required" }).to_string().into(),
                ))
                .await
                .expect("send auth_required");
            let auth = socket
                .next()
                .await
                .expect("auth frame")
                .expect("valid auth frame");
            assert!(
                matches!(auth, Message::Text(ref text) if serde_json::from_str::<JsonValue>(text).expect("auth JSON")["type"] == "auth")
            );
            socket
                .send(Message::Text(
                    json!({ "type": "auth_ok" }).to_string().into(),
                ))
                .await
                .expect("send auth_ok");
            let request = socket
                .next()
                .await
                .expect("request frame")
                .expect("valid request frame");
            let Message::Text(request) = request else {
                panic!("expected text request")
            };
            let request: JsonValue = serde_json::from_str(&request).expect("request JSON");
            assert_eq!(request["type"], "get_states");
            socket
                .send(Message::Text(json!({
                    "id": request["id"],
                    "type": "result",
                    "success": true,
                    "result": [{ "entity_id": "light.kitchen", "state": "on", "attributes": {} }]
                }).to_string().into()))
                .await
                .expect("send states result");
        });

        let states =
            HomeAssistantClient::new(format!("ws://{address}/api/websocket"), "test-token".into())
                .states()
                .await
                .expect("states response");
        assert_eq!(states.len(), 1);
        assert_eq!(states[0].full()["entity_id"], "light.kitchen");
        server.await.expect("server task");
    }

    #[tokio::test]
    async fn subscribes_to_state_changes_and_caches_events() {
        let listener = TcpListener::bind("127.0.0.1:0")
            .await
            .expect("bind mock server");
        let address = listener.local_addr().expect("mock server address");
        let server = tokio::spawn(async move {
            let (stream, _) = listener.accept().await.expect("accept client");
            let mut socket = accept_async(stream).await.expect("upgrade websocket");
            socket
                .send(Message::Text(
                    json!({ "type": "auth_required" }).to_string().into(),
                ))
                .await
                .expect("send auth_required");
            socket
                .next()
                .await
                .expect("auth frame")
                .expect("valid auth frame");
            socket
                .send(Message::Text(
                    json!({ "type": "auth_ok" }).to_string().into(),
                ))
                .await
                .expect("send auth_ok");
            let request = socket
                .next()
                .await
                .expect("subscription frame")
                .expect("valid subscription frame");
            let Message::Text(request) = request else {
                panic!("expected text subscription")
            };
            let request: JsonValue = serde_json::from_str(&request).expect("subscription JSON");
            assert_eq!(request["type"], "subscribe_events");
            assert_eq!(request["event_type"], "state_changed");
            socket
                .send(Message::Text(
                    json!({ "id": request["id"], "type": "result", "success": true, "result": null })
                        .to_string()
                        .into(),
                ))
                .await
                .expect("confirm subscription");
            socket
                .send(Message::Text(
                    json!({
                        "id": request["id"],
                        "type": "event",
                        "event": {
                            "event_type": "state_changed",
                            "data": {
                                "entity_id": "sensor.temperature",
                                "old_state": { "state": "20" },
                                "new_state": { "state": "21" }
                            }
                        }
                    })
                    .to_string()
                    .into(),
                ))
                .await
                .expect("send state change");
        });

        let cache = state_change_cache();
        let client =
            HomeAssistantClient::new(format!("ws://{address}/api/websocket"), "test-token".into());
        assert!(client.listen_state_changes(cache.clone()).await.is_err());
        let events = cache.lock().await;
        assert_eq!(events.len(), 1);
        assert_eq!(events[0]["data"]["entity_id"], "sensor.temperature");
        server.await.expect("server task");
    }

    #[test]
    fn includes_home_assistant_error_code_and_message() {
        let error = service_error(&json!({
            "error": { "code": "service_validation_error", "message": "Unsupported mode" }
        }));
        assert!(format!("{error}").contains("service_validation_error"));
        assert!(format!("{error}").contains("Unsupported mode"));
    }
}
