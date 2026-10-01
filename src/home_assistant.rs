use astra_plugin_sdk::ToolError;
use astra_plugin_sdk::prelude::*;
use futures_util::{SinkExt, StreamExt};
use std::{
    collections::{HashMap, VecDeque},
    sync::Arc,
    time::Duration,
};
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
    pub async fn dashboard_states(&self) -> Result<Vec<Entity>, ToolError> {
        tokio::time::timeout(
            Duration::from_secs(REQUEST_TIMEOUT_SECONDS),
            self.dashboard_states_inner(),
        )
        .await
        .map_err(|_| {
            ToolError::Timeout("Истекло общее время выполнения команды Home Assistant".into())
        })?
    }
    async fn dashboard_states_inner(&self) -> Result<Vec<Entity>, ToolError> {
        let mut connection = self.connect().await?;
        let states = connection.request(json!({ "type": "get_states" })).await?;
        let areas = connection
            .request(json!({ "type": "config/area_registry/list" }))
            .await?;
        let devices = connection
            .request(json!({ "type": "config/device_registry/list" }))
            .await?;
        let entities = connection
            .request(json!({ "type": "config/entity_registry/list" }))
            .await?;
        let areas = registry_array(&areas, "config/area_registry/list")?;
        let devices = registry_array(&devices, "config/device_registry/list")?;
        let entities = registry_array(&entities, "config/entity_registry/list")?;
        let area_metadata: HashMap<_, _> = areas
            .iter()
            .filter_map(|area| {
                Some((
                    area["area_id"].as_str()?.to_owned(),
                    (
                        area["name"].as_str()?.to_owned(),
                        area["icon"].as_str().map(ToOwned::to_owned),
                    ),
                ))
            })
            .collect();
        let device_areas: HashMap<_, _> = devices
            .iter()
            .filter_map(|device| {
                Some((
                    device["id"].as_str()?.to_owned(),
                    device["area_id"].as_str()?.to_owned(),
                ))
            })
            .collect();
        let entity_areas: HashMap<_, _> = entities
            .iter()
            .filter_map(|entity| {
                let entity_id = entity["entity_id"].as_str()?.to_owned();
                let area_id = entity["area_id"]
                    .as_str()
                    .map(ToOwned::to_owned)
                    .or_else(|| {
                        entity["device_id"]
                            .as_str()
                            .and_then(|device_id| device_areas.get(device_id).cloned())
                    })?;
                Some((entity_id, area_id))
            })
            .collect();
        let entity_platforms: HashMap<_, _> = entities
            .iter()
            .filter_map(|entity| {
                Some((
                    entity["entity_id"].as_str()?.to_owned(),
                    entity["platform"].as_str()?.to_owned(),
                ))
            })
            .collect();
        let entity_icons: HashMap<_, _> = entities
            .iter()
            .filter_map(|entity| {
                Some((
                    entity["entity_id"].as_str()?.to_owned(),
                    entity["icon"].as_str()?.to_owned(),
                ))
            })
            .collect();
        let entity_noise: HashMap<_, _> = entities
            .iter()
            .filter_map(|entity| {
                let entity_id = entity["entity_id"].as_str()?.to_owned();
                let category = entity["entity_category"].as_str();
                Some((
                    entity_id,
                    entity["hidden_by"].is_string()
                        || entity["disabled_by"].is_string()
                        || matches!(category, Some("config") | Some("diagnostic")),
                ))
            })
            .collect();
        let states = registry_array(&states, "get_states")?;
        Ok(states
            .iter()
            .cloned()
            .map(|mut value| {
                if let Some(entity_id) = value["entity_id"].as_str().map(ToOwned::to_owned) {
                    if let Some(platform) = entity_platforms.get(&entity_id) {
                        value["platform"] = json!(platform);
                    }
                    if entity_noise.get(&entity_id) == Some(&true) {
                        value["ui_hidden"] = json!(true);
                    }
                    let attribute_icon = value["attributes"]["icon"].clone();
                    if attribute_icon.is_null() {
                        if let Some(icon) = entity_icons.get(&entity_id) {
                            value["icon"] = json!(icon);
                        }
                    } else {
                        value["icon"] = attribute_icon;
                    }
                    if let Some(area_id) = entity_areas.get(&entity_id) {
                        value["area_id"] = json!(area_id);
                        let (area_name, area_icon) = area_metadata
                            .get(area_id)
                            .cloned()
                            .unwrap_or_else(|| (area_id.clone(), None));
                        value["area_name"] = json!(area_name);
                        if let Some(area_icon) = area_icon {
                            value["area_icon"] = json!(area_icon);
                        }
                    }
                }
                Entity { value }
            })
            .collect())
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
    #[cfg(test)]
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
    #[cfg(test)]
    pub fn from_value(value: JsonValue) -> Self {
        Self { value }
    }

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
        let state = if self.value["attributes"]["device_class"] == "temperature" {
            self.value["state"]
                .as_str()
                .and_then(|value| value.parse::<f64>().ok())
                .map(|value| json!(format!("{value:.1}")))
                .unwrap_or_else(|| self.value["state"].clone())
        } else {
            self.value["state"].clone()
        };
        json!({
            "entity_id": self.value["entity_id"],
            "friendly_name": self.value["attributes"]["friendly_name"],
            "state": state,
            "area_id": self.value["area_id"],
            "area_name": self.value["area_name"],
            "area_icon": self.value["area_icon"],
            "platform": self.value["platform"],
            "ui_hidden": self.value["ui_hidden"],
            "icon": self.value["icon"],
            "brightness": self.value["attributes"]["brightness"],
            "unit_of_measurement": self.value["attributes"]["unit_of_measurement"],
            "device_class": self.value["attributes"]["device_class"]
        })
    }
    pub fn full(&self) -> JsonValue {
        self.value.clone()
    }
}

fn registry_array<'a>(
    value: &'a JsonValue,
    request: &str,
) -> Result<&'a Vec<JsonValue>, ToolError> {
    value.as_array().ok_or_else(|| {
        ToolError::Internal(format!(
            "Home Assistant вернул некорректный ответ {request}"
        ))
    })
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

    #[test]
    fn summary_formats_temperature_to_one_decimal() {
        let temperature = Entity::from_value(json!({
            "entity_id": "sensor.temperature",
            "state": "21.26",
            "attributes": {
                "device_class": "temperature",
                "unit_of_measurement": "°C"
            }
        }));
        assert_eq!(temperature.summary()["state"], "21.3");

        let negative_temperature = Entity::from_value(json!({
            "entity_id": "sensor.outdoor_temperature",
            "state": "-3.24",
            "attributes": { "device_class": "temperature" }
        }));
        assert_eq!(negative_temperature.summary()["state"], "-3.2");

        let whole_temperature = Entity::from_value(json!({
            "entity_id": "sensor.room_temperature",
            "state": "21",
            "attributes": { "device_class": "temperature" }
        }));
        assert_eq!(whole_temperature.summary()["state"], "21.0");
    }

    #[test]
    fn summary_keeps_non_numeric_temperature_states_and_other_sensors_unchanged() {
        let unavailable = Entity::from_value(json!({
            "entity_id": "sensor.temperature",
            "state": "unavailable",
            "attributes": { "device_class": "temperature" }
        }));
        assert_eq!(unavailable.summary()["state"], "unavailable");

        let humidity = Entity::from_value(json!({
            "entity_id": "sensor.humidity",
            "state": "45.678",
            "attributes": { "device_class": "humidity" }
        }));
        assert_eq!(humidity.summary()["state"], "45.678");
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

    #[tokio::test]
    async fn dashboard_states_assigns_entities_to_home_assistant_areas() {
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
            let responses = [
                (
                    "get_states",
                    json!([
                        { "entity_id": "light.kitchen", "state": "on", "attributes": {} },
                        { "entity_id": "sensor.temperature", "state": "21", "attributes": {} },
                        { "entity_id": "sensor.uptime", "state": "1", "attributes": {} },
                        { "entity_id": "automation.welcome", "state": "on", "attributes": {} }
                    ]),
                ),
                (
                    "config/area_registry/list",
                    json!([{ "area_id": "kitchen", "name": "Кухня", "icon": "mdi:food-fork-drink" }]),
                ),
                (
                    "config/device_registry/list",
                    json!([{ "id": "device-1", "area_id": "kitchen" }]),
                ),
                (
                    "config/entity_registry/list",
                    json!([
                        { "entity_id": "light.kitchen", "area_id": "kitchen", "icon": "mdi:lightbulb-outline" },
                        { "entity_id": "sensor.temperature", "device_id": "device-1" },
                        { "entity_id": "sensor.uptime", "platform": "homeassistant" },
                        { "entity_id": "automation.welcome", "platform": "automation" }
                    ]),
                ),
            ];
            for (expected_type, result) in responses {
                let request = socket
                    .next()
                    .await
                    .expect("registry request")
                    .expect("valid registry request");
                let Message::Text(request) = request else {
                    panic!("expected text request")
                };
                let request: JsonValue = serde_json::from_str(&request).expect("request JSON");
                assert_eq!(request["type"], expected_type);
                socket
                    .send(Message::Text(
                        json!({
                            "id": request["id"],
                            "type": "result",
                            "success": true,
                            "result": result
                        })
                        .to_string()
                        .into(),
                    ))
                    .await
                    .expect("send registry result");
            }
        });

        let states =
            HomeAssistantClient::new(format!("ws://{address}/api/websocket"), "test-token".into())
                .dashboard_states()
                .await
                .expect("dashboard states");
        assert_eq!(states.len(), 4);
        assert_eq!(states[0].full()["area_name"], "Кухня");
        assert_eq!(states[0].summary()["area_icon"], "mdi:food-fork-drink");
        assert_eq!(states[0].summary()["icon"], "mdi:lightbulb-outline");
        assert_eq!(states[1].full()["area_name"], "Кухня");
        assert_eq!(states[2].full()["platform"], "homeassistant");
        assert_eq!(states[3].full()["platform"], "automation");
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
