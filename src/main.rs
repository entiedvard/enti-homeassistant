mod home_assistant;

use astra_plugin_sdk::prelude::*;
use astra_plugin_sdk::{Config, ToolError};
use home_assistant::{HomeAssistantClient, ServiceCall, StateChangeCache, state_change_cache};
use std::sync::Mutex;
use tokio::task::JoinHandle;
use tokio::time::{Duration, sleep};

#[astra::config]
#[derive(Default, Clone)]
struct Settings {
    /// URL WebSocket API Home Assistant, оканчивающийся на `/api/websocket`.
    home_assistant_url: String,
    /// Long-Lived Access Token, созданный в профиле Home Assistant.
    access_token: String,
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
    /// Часть отображаемого имени или entity_id. Пустая строка возвращает сущности выбранного домена.
    #[serde(default)]
    query: String,
    /// Необязательный домен Home Assistant, например light, switch или sensor.
    #[serde(default)]
    domain: String,
    /// Максимальное число результатов (от 1 до 100).
    #[serde(default = "default_limit")]
    limit: u32,
}

#[astra::args]
struct EntityIdArgs {
    /// Точный идентификатор сущности, например light.kitchen.
    entity_id: String,
}

#[astra::args]
struct ControlEntityArgs {
    /// Точный идентификатор сущности, например light.kitchen.
    entity_id: String,
    /// Операция: turn_on, turn_off или toggle.
    operation: String,
    /// Необязательные параметры сервиса как JSON-объект, например {"brightness_pct":50}.
    #[serde(default = "empty_object")]
    data_json: String,
}

#[astra::args]
struct CallServiceArgs {
    /// Домен сервиса, например light.
    domain: String,
    /// Имя сервиса, например turn_on.
    service: String,
    /// Необязательная точная сущность, к которой применяется сервис.
    #[serde(default)]
    entity_id: String,
    /// Параметры сервиса как JSON-объект.
    #[serde(default = "empty_object")]
    data_json: String,
}

#[astra::args]
struct RecentStateChangesArgs {
    /// Максимальное число последних изменений, от 1 до 100.
    #[serde(default = "default_limit")]
    limit: u32,
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
        if let Ok((url, token)) = config.connection() {
            self.start_subscription(url, token);
        }
    }

    #[hook]
    async fn on_shutdown(&self, _ctx: &PluginContext) {
        self.stop_subscription();
    }

    /// Найди сущности Home Assistant по имени или entity_id. Используй перед управлением, если точный entity_id неизвестен.
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

    /// Получи текущее состояние и атрибуты сущности Home Assistant по точному entity_id.
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

    /// Включи, выключи или переключи одну сущность Home Assistant. Для яркости и прочих параметров передай JSON в data_json.
    #[tool]
    async fn control_entity(
        &self,
        _ctx: &PluginContext,
        args: ControlEntityArgs,
    ) -> Result<String, ToolError> {
        let entity_id = required("entity_id", &args.entity_id)?;
        let operation = match args.operation.trim() {
            "turn_on" | "turn_off" | "toggle" => args.operation.trim(),
            _ => {
                return Err(ToolError::BadArguments(
                    "operation должен быть turn_on, turn_off или toggle".into(),
                ));
            }
        };
        let domain = entity_id
            .split_once('.')
            .map(|(domain, _)| domain)
            .filter(|domain| !domain.is_empty())
            .ok_or_else(|| {
                ToolError::BadArguments("entity_id должен иметь вид domain.object_id".into())
            })?;
        let client = self.client()?;
        client
            .call_service(ServiceCall::for_entity(
                domain,
                operation,
                entity_id,
                parse_object(&args.data_json)?,
            ))
            .await?;
        json_pretty(
            json!({ "entity_id": entity_id, "operation": operation, "state": client.state(entity_id).await?.full() }),
        )
    }

    /// Вызови произвольный сервис Home Assistant, когда стандартных операций управления недостаточно.
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

    /// Получи последние изменения состояний из постоянной подписки Home Assistant. Подписка запускается после сохранения корректных настроек и хранит до 100 событий.
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

    fn start_subscription(&self, url: String, token: String) {
        let cache = self.state_changes.clone();
        let task = tokio::spawn(async move {
            loop {
                let client = HomeAssistantClient::new(url.clone(), token.clone());
                let _ = client.listen_state_changes(cache.clone()).await;
                sleep(Duration::from_secs(5)).await;
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
        assert_eq!(harness.tools().await.len(), 5);
        harness
            .assert_schema_matches::<SearchEntitiesArgs>("search_entities")
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
    #[test]
    fn only_json_objects_are_accepted_as_service_data() {
        assert!(parse_object("{\"brightness_pct\":50}").is_ok());
        assert!(parse_object("[]").is_err());
    }
}
