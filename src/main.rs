mod home_assistant;

use astra_plugin_sdk::prelude::*;
use astra_plugin_sdk::{Config, ToolError};
use home_assistant::{HomeAssistantClient, ServiceCall, StateChangeCache, state_change_cache};
use std::{collections::HashMap, sync::Mutex};
use tokio::task::JoinHandle;
use tokio::time::{Duration, sleep};

#[astra::config]
#[derive(Default, Clone)]
struct Settings {
    /// URL WebSocket API Home Assistant, оканчивающийся на `/api/websocket`.
    home_assistant_url: String,
    /// Long-Lived Access Token, созданный в профиле Home Assistant.
    access_token: String,
    /// Включает опрос состояний раз в 10 минут для журнала.
    #[serde(default)]
    track_state_changes: bool,
}

impl Settings {
    fn connection(&self) -> Result<(String, String), ToolError> {
        let url = self.home_assistant_url.trim();
        if url.is_empty() {
            return Err(ToolError::not_configured("home_assistant_url"));
        }
        if !url.starts_with("ws://") && !url.starts_with("wss://") {
            return Err(ToolError::BadArguments(
                "URL должен начинаться с ws:// или wss://".into(),
            ));
        }
        let path_without_query = url.split(['?', '#']).next().unwrap_or(url);
        if !path_without_query.ends_with("/api/websocket") {
            return Err(ToolError::BadArguments(
                "URL должен оканчиваться на /api/websocket".into(),
            ));
        }
        let token = self.access_token.trim();
        if token.is_empty() {
            return Err(ToolError::not_configured("access_token"));
        }
        Ok((url.to_owned(), token.to_owned()))
    }
}

#[astra::args]
struct SearchEntitiesArgs {
    /// Поисковая строка для имени или entity_id. Это только поиск: команда ничего не включает и не изменяет. Для текущей температуры, влажности, давления и других показаний используй get_sensor_readings.
    #[serde(default)]
    query: String,
    /// Необязательный домен Home Assistant: light, switch, cover, scene, sensor и другие.
    #[serde(default)]
    domain: String,
    /// Максимальное число найденных сущностей, от 1 до 100.
    #[serde(default = "default_limit")]
    limit: u32,
}

#[astra::args]
struct SensorReadingsArgs {
    /// Название датчика или часть entity_id, например kitchen, bedroom или temperature. Пустая строка возвращает все доступные показания.
    #[serde(default)]
    query: String,
    /// Тип показания Home Assistant: temperature, humidity, pressure, illuminance, battery и другие device_class. Оставь пустым для любого типа.
    #[serde(default)]
    device_class: String,
    /// Максимальное число показаний, от 1 до 100.
    #[serde(default = "default_limit")]
    limit: u32,
}

#[astra::args]
struct EntityIdArgs {
    /// Полный точный entity_id одной сущности, например light.kitchen или sensor.kitchen_temperature. Для поиска показаний по названию используй get_sensor_readings.
    entity_id: String,
}

#[astra::args]
struct ControlEntityArgs {
    /// Полный точный entity_id управляемой сущности, например light.kitchen.
    entity_id: String,
    /// Стандартная операция управления: turn_on, turn_off или toggle для light/switch; open_cover, close_cover, stop_cover или set_cover_position для cover; activate_scene для scene. Эта команда изменяет состояние устройства.
    operation: String,
    /// Необязательные данные стандартного сервиса в JSON-объекте, например {"brightness_pct":50}. Для нестандартного сервиса используй call_service.
    #[serde(default = "empty_object")]
    data_json: String,
}

#[astra::args]
struct CallServiceArgs {
    /// Домен произвольного сервиса Home Assistant, например light, media_player или notify.
    domain: String,
    /// Точное имя произвольного сервиса, например turn_on, volume_set или send_message. Для обычного включения/выключения используй control_entity.
    service: String,
    /// Необязательный точный entity_id, к которому применяется сервис. Оставь пустым для вызова сервиса без конкретной сущности.
    #[serde(default)]
    entity_id: String,
    /// Аргументы произвольного сервиса в виде JSON-объекта.
    #[serde(default = "empty_object")]
    data_json: String,
}

#[astra::args]
struct RecentStateChangesArgs {
    /// Максимальное число последних событий state_changed, от 1 до 100. Это только чтение журнала, не управление устройствами.
    #[serde(default = "default_limit")]
    limit: u32,
}

#[astra::args]
struct UiEntitiesArgs {
    /// Необязательная строка поиска по имени или entity_id.
    #[serde(default)]
    query: String,
    /// Необязательный домен Home Assistant, например light, switch или sensor.
    #[serde(default)]
    domain: String,
}

#[astra::args]
struct UiControlArgs {
    /// Точный идентификатор сущности.
    entity_id: String,
    /// Операция: turn_on, turn_off, toggle, open_cover, close_cover, stop_cover или activate_scene.
    operation: String,
    /// Необязательная яркость света в процентах от 1 до 100.
    #[serde(default)]
    brightness_pct: Option<u8>,
}

fn default_limit() -> u32 {
    20
}
fn empty_object() -> String {
    "{}".into()
}

struct HomeAssistantPlugin {
    config: Config<Settings>,
    state_changes: StateChangeCache,
    subscription_task: Mutex<Option<JoinHandle<()>>>,
}

impl Default for HomeAssistantPlugin {
    fn default() -> Self {
        Self {
            config: Config::default(),
            state_changes: state_change_cache(),
            subscription_task: Mutex::new(None),
        }
    }
}

#[astra::plugin(config = Settings)]
impl HomeAssistantPlugin {
    #[hook]
    async fn on_start(&self, ctx: &PluginContext) -> anyhow::Result<()> {
        ctx.host().log_info("Home Assistant plugin started").await?;
        Ok(())
    }

    #[hook]
    async fn on_config(&self, _ctx: &PluginContext, config: Settings) {
        self.config.store(config.clone());
        self.stop_subscription();
        self.state_changes.lock().await.clear();
        if config.track_state_changes {
            if let Ok((url, token)) = config.connection() {
                self.start_state_polling(url, token);
            }
        }
    }

    #[hook]
    async fn on_shutdown(&self, _ctx: &PluginContext) {
        self.stop_subscription();
    }

    #[hook]
    async fn ui_contributions(&self) -> Vec<UiContribution> {
        vec![
            UiContribution::page("home-assistant", "Home Assistant", "home-assistant-v11.js")
                .with_icon_svg(include_str!("../ui/home-assistant-icon.svg")),
        ]
    }

    /// ПОИСК: найди сущности Home Assistant по имени или entity_id. Используй перед управлением, если точный entity_id неизвестен. Для запросов текущей температуры, влажности, давления и других показаний ОБЯЗАТЕЛЬНО предпочитай get_sensor_readings. Не изменяет состояние устройств.
    #[tool]
    async fn search_entities(
        &self,
        _ctx: &PluginContext,
        args: SearchEntitiesArgs,
    ) -> Result<String, ToolError> {
        if args.limit == 0 || args.limit > 100 {
            return Err(ToolError::BadArguments(
                "limit должен быть в диапазоне от 1 до 100".into(),
            ));
        }
        let query = args.query.trim().to_lowercase();
        let domain = args.domain.trim().to_lowercase();
        let results: Vec<_> = self
            .client()?
            .states()
            .await?
            .into_iter()
            .filter(|entity| domain.is_empty() || entity.domain() == domain)
            .filter(|entity| query.is_empty() || entity.matches(&query))
            .take(args.limit as usize)
            .map(|entity| entity.summary())
            .collect();
        json_pretty(json!({ "count": results.len(), "entities": results }))
    }

    /// ПОКАЗАНИЯ ДАТЧИКОВ: используй этот инструмент, когда пользователь спрашивает текущую температуру, влажность, давление, освещённость, заряд батареи или другое значение датчика Home Assistant. Возвращает только sensor/binary_sensor с state, unit_of_measurement и device_class. Ничего не изменяет и не вызывает внутренние инструменты Astra. Не используй control_entity или call_service для чтения показаний.
    #[tool]
    async fn get_sensor_readings(
        &self,
        _ctx: &PluginContext,
        args: SensorReadingsArgs,
    ) -> Result<String, ToolError> {
        if args.limit == 0 || args.limit > 100 {
            return Err(ToolError::BadArguments(
                "limit должен быть в диапазоне от 1 до 100".into(),
            ));
        }
        let query = args.query.trim().to_lowercase();
        let device_class = args.device_class.trim().to_lowercase();
        let readings: Vec<_> = self
            .client()?
            .states()
            .await?
            .into_iter()
            .filter(|entity| matches!(entity.domain().as_str(), "sensor" | "binary_sensor"))
            .filter(|entity| query.is_empty() || entity.matches(&query))
            .filter(|entity| {
                device_class.is_empty()
                    || entity.full()["attributes"]["device_class"]
                        .as_str()
                        .map(|value| value.eq_ignore_ascii_case(&device_class))
                        .unwrap_or(false)
            })
            .take(args.limit as usize)
            .map(|entity| entity.summary())
            .collect();
        json_pretty(json!({ "count": readings.len(), "readings": readings }))
    }

    /// ЧТЕНИЕ: получи текущее состояние и все атрибуты одной сущности Home Assistant по точному entity_id. Не изменяет устройство.
    #[tool]
    async fn get_entity_state(
        &self,
        _ctx: &PluginContext,
        args: EntityIdArgs,
    ) -> Result<String, ToolError> {
        json_pretty(
            self.client()?
                .state(required("entity_id", &args.entity_id)?)
                .await?
                .full(),
        )
    }

    /// УПРАВЛЕНИЕ: измени одну сущность Home Assistant стандартной операцией. Используй для включения, выключения, переключения света/выключателя, управления шторами и запуска сцен. Для нестандартных сервисов используй call_service.
    #[tool]
    async fn control_entity(
        &self,
        _ctx: &PluginContext,
        args: ControlEntityArgs,
    ) -> Result<String, ToolError> {
        let entity_id = required("entity_id", &args.entity_id)?;
        let operation = valid_operation(&args.operation)?;
        let domain = entity_id
            .split_once('.')
            .map(|(domain, _)| domain)
            .filter(|domain| !domain.is_empty())
            .ok_or_else(|| {
                ToolError::BadArguments("entity_id должен иметь вид domain.object_id".into())
            })?;
        valid_operation_for_domain(domain, operation)?;
        let client = self.client()?;
        let service = if operation == "activate_scene" {
            "turn_on"
        } else {
            operation
        };
        client
            .call_service(ServiceCall::for_entity(
                domain,
                service,
                entity_id,
                parse_object(&args.data_json)?,
            ))
            .await?;
        json_pretty(
            json!({ "entity_id": entity_id, "operation": operation, "state": client.state(entity_id).await?.full() }),
        )
    }

    /// ПРОИЗВОЛЬНЫЙ СЕРВИС: вызови домен/сервис Home Assistant напрямую с JSON-аргументами. Используй только когда control_entity не подходит; для обычного включения/выключения используй control_entity.
    #[tool]
    async fn call_service(
        &self,
        _ctx: &PluginContext,
        args: CallServiceArgs,
    ) -> Result<String, ToolError> {
        let domain = required("domain", &args.domain)?;
        let service = required("service", &args.service)?;
        let entity_id = args.entity_id.trim();
        self.client()?
            .call_service(ServiceCall::new(
                domain,
                service,
                entity_id,
                parse_object(&args.data_json)?,
            ))
            .await?;
        json_pretty(
            json!({ "success": true, "domain": domain, "service": service, "entity_id": entity_id }),
        )
    }

    /// ЖУРНАЛ: получи последние изменения состояний из подписки Home Assistant. Только чтение, без управления устройствами. Подписка запускается после сохранения корректных настроек и хранит до 100 событий.
    #[tool]
    async fn get_recent_state_changes(
        &self,
        _ctx: &PluginContext,
        args: RecentStateChangesArgs,
    ) -> Result<String, ToolError> {
        if args.limit == 0 || args.limit > 100 {
            return Err(ToolError::BadArguments(
                "limit должен быть в диапазоне от 1 до 100".into(),
            ));
        }
        let events = self.state_changes.lock().await;
        let changes: Vec<_> = events
            .iter()
            .rev()
            .take(args.limit as usize)
            .cloned()
            .collect();
        json_pretty(json!({ "count": changes.len(), "events": changes }))
    }

    /// Возвращает сущности для панели Home Assistant.
    #[ui_call]
    async fn list_entities_for_ui(
        &self,
        _ctx: &PluginContext,
        args: UiEntitiesArgs,
    ) -> Result<String, ToolError> {
        let query = args.query.trim().to_lowercase();
        let domain = args.domain.trim().to_lowercase();
        let entities: Vec<_> = self
            .client()?
            .dashboard_states()
            .await?
            .into_iter()
            .filter(visible_in_dashboard)
            .filter(|entity| domain.is_empty() || entity.domain() == domain)
            .filter(|entity| query.is_empty() || entity.matches(&query))
            .map(|entity| entity.summary())
            .collect();
        json_pretty(json!({ "entities": entities }))
    }

    /// Выполняет простую операцию над light или switch из панели.
    #[ui_call]
    async fn control_entity_from_ui(
        &self,
        _ctx: &PluginContext,
        args: UiControlArgs,
    ) -> Result<String, ToolError> {
        let entity_id = required("entity_id", &args.entity_id)?;
        let operation = valid_operation(&args.operation)?;
        let domain = entity_domain(entity_id)?;
        if domain != "light" && domain != "switch" && domain != "cover" && domain != "scene" {
            return Err(ToolError::BadArguments(
                "Панель поддерживает управление только light, switch, cover и scene".into(),
            ));
        }
        valid_operation_for_domain(domain, operation)?;
        let data = match args.brightness_pct {
            Some(brightness_pct) if domain == "light" && (1..=100).contains(&brightness_pct) => {
                json!({ "brightness_pct": brightness_pct })
            }
            Some(_) => {
                return Err(ToolError::BadArguments(
                    "brightness_pct должен быть от 1 до 100 и применим только к light".into(),
                ));
            }
            None => json!({}),
        };
        let service = if operation == "activate_scene" {
            "turn_on"
        } else {
            operation
        };
        self.client()?
            .call_service(ServiceCall::for_entity(domain, service, entity_id, data))
            .await?;
        json_pretty(json!({ "success": true, "entity_id": entity_id, "operation": operation }))
    }

    /// Возвращает последние события state_changed для панели.
    #[ui_call]
    async fn recent_state_changes_for_ui(
        &self,
        _ctx: &PluginContext,
        args: RecentStateChangesArgs,
    ) -> Result<String, ToolError> {
        if args.limit == 0 || args.limit > 100 {
            return Err(ToolError::BadArguments(
                "limit должен быть в диапазоне от 1 до 100".into(),
            ));
        }
        let events = self.state_changes.lock().await;
        let events: Vec<_> = events
            .iter()
            .rev()
            .take(args.limit as usize)
            .cloned()
            .collect();
        json_pretty(json!({ "events": events }))
    }
}

impl HomeAssistantPlugin {
    fn client(&self) -> Result<HomeAssistantClient, ToolError> {
        let config = self.config.get();
        let (url, token) = config.connection()?;
        Ok(HomeAssistantClient::new(url, token))
    }

    fn stop_subscription(&self) {
        if let Ok(mut task) = self.subscription_task.lock() {
            if let Some(task) = task.take() {
                task.abort();
            }
        }
    }

    fn start_state_polling(&self, url: String, token: String) {
        let cache = self.state_changes.clone();
        let task = tokio::spawn(async move {
            let mut previous: HashMap<String, JsonValue> = HashMap::new();
            loop {
                let client = HomeAssistantClient::new(url.clone(), token.clone());
                if let Ok(states) = client.states().await {
                    let mut current = HashMap::with_capacity(states.len());
                    let mut changes = Vec::new();
                    for entity in states {
                        let state = entity.full();
                        let Some(entity_id) = state["entity_id"].as_str() else {
                            continue;
                        };
                        if let Some(old_state) = previous.get(entity_id)
                            && old_state["state"] != state["state"]
                        {
                            changes.push(json!({
                                "event_type": "state_changed",
                                "data": {
                                    "entity_id": entity_id,
                                    "old_state": old_state,
                                    "new_state": state
                                }
                            }));
                        }
                        current.insert(entity_id.to_owned(), state);
                    }
                    previous = current;
                    if !changes.is_empty() {
                        let mut events = cache.lock().await;
                        for change in changes {
                            if events.len() == 100 {
                                events.pop_front();
                            }
                            events.push_back(change);
                        }
                    }
                }
                sleep(Duration::from_secs(10 * 60)).await;
            }
        });
        if let Ok(mut current) = self.subscription_task.lock() {
            *current = Some(task);
        } else {
            task.abort();
        }
    }
}

fn required<'a>(field: &str, value: &'a str) -> Result<&'a str, ToolError> {
    let value = value.trim();
    if value.is_empty() {
        Err(ToolError::BadArguments(format!(
            "{field} не должен быть пустым"
        )))
    } else {
        Ok(value)
    }
}
fn valid_operation(value: &str) -> Result<&str, ToolError> {
    match value.trim() {
        "turn_on" | "turn_off" | "toggle" | "open_cover" | "close_cover" | "stop_cover"
        | "set_cover_position" | "activate_scene" => Ok(value.trim()),
        _ => Err(ToolError::BadArguments(
            "operation должен быть turn_on, turn_off, toggle, open_cover, close_cover, stop_cover, set_cover_position или activate_scene".into(),
        )),
    }
}
fn valid_operation_for_domain(domain: &str, operation: &str) -> Result<(), ToolError> {
    let valid = match domain {
        "cover" => matches!(
            operation,
            "open_cover" | "close_cover" | "stop_cover" | "set_cover_position"
        ),
        "scene" => operation == "activate_scene",
        _ => matches!(operation, "turn_on" | "turn_off" | "toggle"),
    };
    if valid {
        Ok(())
    } else {
        Err(ToolError::BadArguments(format!(
            "Операция {operation} не поддерживается для домена {domain}"
        )))
    }
}
fn entity_domain(entity_id: &str) -> Result<&str, ToolError> {
    entity_id
        .split_once('.')
        .map(|(domain, _)| domain)
        .filter(|domain| !domain.is_empty())
        .ok_or_else(|| {
            ToolError::BadArguments("entity_id должен иметь вид domain.object_id".into())
        })
}

fn visible_in_dashboard(entity: &home_assistant::Entity) -> bool {
    let domain = entity.domain();
    domain != "automation"
        && (domain == "scene" || entity.full()["platform"].as_str() != Some("homeassistant"))
        && entity.full()["ui_hidden"].as_bool() != Some(true)
}

fn parse_object(input: &str) -> Result<JsonValue, ToolError> {
    let value: JsonValue = serde_json::from_str(input).map_err(|_| {
        ToolError::BadArguments("data_json должен быть корректным JSON-объектом".into())
    })?;
    if value.is_object() {
        Ok(value)
    } else {
        Err(ToolError::BadArguments(
            "data_json должен быть JSON-объектом".into(),
        ))
    }
}
fn json_pretty(value: JsonValue) -> Result<String, ToolError> {
    serde_json::to_string_pretty(&value).map_err(ToolError::internal)
}

astra::main!(HomeAssistantPlugin::default());

#[cfg(test)]
mod tests {
    use super::*;
    use astra_plugin_sdk::testing::Harness;
    #[tokio::test]
    async fn exposes_the_expected_tools_and_schemas() {
        let harness = Harness::new(HomeAssistantPlugin::default())
            .with_config(json!({}))
            .start()
            .await
            .expect("plugin starts");
        assert_eq!(harness.tools().await.len(), 6);
        harness
            .assert_schema_matches::<SearchEntitiesArgs>("search_entities")
            .await;
        harness
            .assert_schema_matches::<SensorReadingsArgs>("get_sensor_readings")
            .await;
        harness
            .assert_schema_matches::<EntityIdArgs>("get_entity_state")
            .await;
        harness
            .assert_schema_matches::<ControlEntityArgs>("control_entity")
            .await;
        harness
            .assert_schema_matches::<CallServiceArgs>("call_service")
            .await;
        harness
            .assert_schema_matches::<RecentStateChangesArgs>("get_recent_state_changes")
            .await;
        let contributions = harness.ui_contributions().await;
        assert_eq!(contributions.len(), 1);
        assert_eq!(contributions[0].id, "home-assistant");
        assert_eq!(contributions[0].slot, "page.custom");
        assert_eq!(contributions[0].url, "home-assistant-v11.js");
        assert!(harness.health().await.0);
    }
    #[tokio::test]
    async fn reports_missing_connection_settings_without_connecting() {
        let harness = Harness::new(HomeAssistantPlugin::default())
            .with_config(json!({}))
            .start()
            .await
            .expect("plugin starts");
        let error = harness
            .call_tool("search_entities", json!({}))
            .await
            .expect_err("missing settings must be reported");
        assert_eq!(error.config_field(), Some("home_assistant_url"));
    }
    #[tokio::test]
    async fn ui_reports_missing_connection_settings_without_connecting() {
        let harness = Harness::new(HomeAssistantPlugin::default())
            .with_config(json!({}))
            .start()
            .await
            .expect("plugin starts");
        let error = harness
            .ui_call("list_entities_for_ui", json!({}))
            .await
            .expect_err("missing settings must be reported");
        assert_eq!(error.config_field(), Some("home_assistant_url"));
    }
    #[test]
    fn only_json_objects_are_accepted_as_service_data() {
        assert!(parse_object("{\"brightness_pct\":50}").is_ok());
        assert!(parse_object("[]").is_err());
    }

    #[test]
    fn validates_cover_operations_by_domain() {
        assert!(valid_operation_for_domain("cover", "open_cover").is_ok());
        assert!(valid_operation_for_domain("cover", "close_cover").is_ok());
        assert!(valid_operation_for_domain("cover", "stop_cover").is_ok());
        assert!(valid_operation_for_domain("cover", "set_cover_position").is_ok());
        assert!(valid_operation_for_domain("cover", "toggle").is_err());
        assert!(valid_operation_for_domain("light", "open_cover").is_err());
    }

    #[test]
    fn validates_scene_activation_by_domain() {
        assert!(valid_operation_for_domain("scene", "activate_scene").is_ok());
        assert!(valid_operation_for_domain("scene", "turn_on").is_err());
        assert!(valid_operation_for_domain("light", "activate_scene").is_err());
    }

    #[test]
    fn keeps_home_assistant_scenes_visible_in_the_dashboard() {
        let scene = home_assistant::Entity::from_value(json!({
            "entity_id": "scene.good_night",
            "platform": "homeassistant",
            "ui_hidden": false
        }));
        let system_sensor = home_assistant::Entity::from_value(
            json!({ "entity_id": "sensor.uptime", "platform": "homeassistant" }),
        );
        let automation =
            home_assistant::Entity::from_value(json!({ "entity_id": "automation.good_night" }));
        assert!(visible_in_dashboard(&scene));
        assert!(!visible_in_dashboard(&system_sensor));
        assert!(!visible_in_dashboard(&automation));
    }
}
