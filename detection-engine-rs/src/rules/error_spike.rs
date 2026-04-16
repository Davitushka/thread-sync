use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;

use crate::alert::{Alert, AlertSeverity};
use crate::event::Event;
use crate::state_store::StateStore;

use super::{Rule, StatefulRule, format_duration};

pub struct ErrorSpikeRule {
    pub threshold: i64,
    pub window: Duration,
}

impl ErrorSpikeRule {
    pub fn new() -> Self {
        Self {
            threshold: 20,
            window: Duration::from_secs(60),
        }
    }

    fn is_server_error(code: i32) -> bool {
        (500..600).contains(&code)
    }
}

impl Rule for ErrorSpikeRule {
    fn id(&self) -> &str {
        "error_spike"
    }
    fn title(&self) -> &str {
        "Server Error Spike — Anomalous 5xx Rate on Endpoint"
    }
    fn match_event(&self, _event: &Event) -> Option<Alert> {
        None
    }
}

#[async_trait]
impl StatefulRule for ErrorSpikeRule {
    async fn evaluate(&self, event: &Event, state: &dyn StateStore) -> Option<Alert> {
        let code = event.status_code?;

        if !Self::is_server_error(code) {
            return None;
        }

        if event.source_type != "dotnet" && event.source_type != "nginx" {
            return None;
        }

        let path = Event::str_val(&event.url_path);
        let key = format!("err:{}:{}", path, Event::str_val(&event.source_ip));
        let count = state.increment(&key, self.window).await.ok()?;

        if count < self.threshold {
            return None;
        }

        // Anti-spam: fire only once per window per key
        let antispan_key = format!("err:fired:{}:{}", path, Event::str_val(&event.source_ip));
        if state.get(&antispan_key).await.unwrap_or(0) > 0 {
            return None;
        }
        let _ = state.increment(&antispan_key, self.window).await;

        let mut context = HashMap::new();
        context.insert("error_count".into(), serde_json::json!(count));
        context.insert(
            "window".into(),
            serde_json::json!(format_duration(self.window)),
        );
        context.insert("url_path".into(), serde_json::json!(path));
        context.insert("status_code".into(), serde_json::json!(code));
        context.insert(
            "http_method".into(),
            serde_json::json!(Event::str_val(&event.http_method)),
        );

        Some(Alert {
            rule_id: self.id().into(),
            rule_title: self.title().into(),
            severity: AlertSeverity::High,
            description: format!(
                "Error spike: {} 5xx responses in {} on {} from {}",
                self.threshold,
                format_duration(self.window),
                path,
                Event::str_val(&event.source_ip),
            ),
            source_ip: event.source_ip.clone(),
            user_id: event.user_id.clone(),
            event_ids: vec![event.event_id.clone()],
            mitre_tags: vec!["T1190".into()],
            fired_at: Utc::now(),
            context,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::rules::test_utils::{MockStateStore, event_with};

    #[tokio::test]
    async fn fires_on_5xx_spike() {
        let rule = ErrorSpikeRule {
            threshold: 3,
            window: Duration::from_secs(60),
        };
        let store = Arc::new(MockStateStore::default());
        let event = event_with(|e| {
            e.source_type = "dotnet".into();
            e.status_code = Some(500);
            e.url_path = Some("/api/orders".into());
            e.source_ip = Some("10.0.0.5".into());
        });

        assert!(rule.evaluate(&event, store.as_ref()).await.is_none());
        assert!(rule.evaluate(&event, store.as_ref()).await.is_none());
        let alert = rule
            .evaluate(&event, store.as_ref())
            .await
            .expect("expected error spike alert");
        assert_eq!(alert.rule_id, "error_spike");
        assert_eq!(alert.severity, AlertSeverity::High);
    }

    #[tokio::test]
    async fn ignores_4xx_and_2xx() {
        let rule = ErrorSpikeRule::new();
        let store = Arc::new(MockStateStore::default());
        let event = event_with(|e| {
            e.status_code = Some(404);
            e.source_ip = Some("10.0.0.5".into());
        });

        let alert = rule.evaluate(&event, store.as_ref()).await;
        assert!(alert.is_none());
    }
}
