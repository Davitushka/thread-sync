use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;

use crate::alert::{Alert, AlertSeverity};
use crate::event::Event;
use crate::state_store::StateStore;

use super::{Rule, StatefulRule, format_duration};

pub struct DataExfiltrationRule {
    pub threshold_events: i64,
    pub window: Duration,
}

impl DataExfiltrationRule {
    pub fn new() -> Self {
        Self {
            threshold_events: 100, // 100 large responses in window
            window: Duration::from_secs(300),
        }
    }

    fn is_large_response(event: &Event) -> bool {
        // Check response_size in metadata or use duration as proxy
        if let Some(size) = event.metadata.get("ResponseSize").and_then(|v| v.as_i64()) {
            return size > 1_000_000; // > 1MB
        }
        // Fallback: check Content-Length header in metadata
        if let Some(size) = event.metadata.get("ContentLength").and_then(|v| v.as_i64()) {
            return size > 1_000_000;
        }
        // If duration_ms is abnormally high, could indicate large transfer
        if let Some(dur) = event.duration_ms {
            return dur > 5000.0; // > 5s response time
        }
        false
    }
}

impl Rule for DataExfiltrationRule {
    fn id(&self) -> &str {
        "data_exfiltration"
    }
    fn title(&self) -> &str {
        "Data Exfiltration — Anomalous Outbound Data Volume"
    }
    fn match_event(&self, _event: &Event) -> Option<Alert> {
        None
    }
}

#[async_trait]
impl StatefulRule for DataExfiltrationRule {
    async fn evaluate(&self, event: &Event, state: &dyn StateStore) -> Option<Alert> {
        if !Self::is_large_response(event) {
            return None;
        }

        if event.status_code != Some(200) {
            return None;
        }

        let ip = event.source_ip.as_ref()?;
        let user_id = Event::str_val(&event.user_id);
        let key = format!("exfil:{}:{}", ip, user_id);
        let count = state.increment(&key, self.window).await.ok()?;

        if count < self.threshold_events {
            return None;
        }

        // Anti-spam: fire only once per window per key
        let antispan_key = format!("exfil:fired:{}:{}", ip, user_id);
        if state.get(&antispan_key).await.unwrap_or(0) > 0 {
            return None;
        }
        if let Err(e) = state.increment(&antispan_key, self.window).await {
            tracing::warn!(error = %e, "antispan increment failed — alert may re-fire");
        }

        let mut context = HashMap::new();
        context.insert("large_response_count".into(), serde_json::json!(count));
        context.insert(
            "window".into(),
            serde_json::json!(format_duration(self.window)),
        );
        context.insert("user_id".into(), serde_json::json!(user_id));
        context.insert(
            "url_path".into(),
            serde_json::json!(Event::str_val(&event.url_path)),
        );

        if let Some(size) = event
            .metadata
            .get("ResponseSize")
            .or_else(|| event.metadata.get("ContentLength"))
        {
            context.insert("response_size".into(), size.clone());
        }

        Some(Alert {
            rule_id: self.id().into(),
            rule_title: self.title().into(),
            severity: AlertSeverity::High,
            description: format!(
                "Data exfiltration suspected: {} large responses to {} (user={}) in {}",
                self.threshold_events,
                ip,
                user_id,
                format_duration(self.window),
            ),
            source_ip: Some(ip.clone()),
            user_id: event.user_id.clone(),
            event_ids: vec![event.event_id.clone()],
            mitre_tags: vec!["T1048".into(), "T1041".into()],
            fired_at: Utc::now(),
            context,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use serde_json::json;

    use super::*;
    use crate::rules::test_utils::{MockStateStore, event_with, set_metadata};

    #[tokio::test]
    async fn fires_on_large_response_spike() {
        let rule = DataExfiltrationRule {
            threshold_events: 2,
            window: Duration::from_secs(300),
        };
        let store = Arc::new(MockStateStore::default());
        let event = event_with(|e| {
            e.status_code = Some(200);
            e.source_ip = Some("10.0.0.5".into());
            e.user_id = Some("alice".into());
            set_metadata(e, "ResponseSize", json!(5_000_000));
        });

        assert!(rule.evaluate(&event, store.as_ref()).await.is_none());
        let alert = rule
            .evaluate(&event, store.as_ref())
            .await
            .expect("expected exfiltration alert");
        assert_eq!(alert.rule_id, "data_exfiltration");
        assert_eq!(alert.severity, AlertSeverity::High);
    }

    #[tokio::test]
    async fn ignores_normal_responses() {
        let rule = DataExfiltrationRule::new();
        let store = Arc::new(MockStateStore::default());
        let event = event_with(|e| {
            e.status_code = Some(200);
            e.source_ip = Some("10.0.0.5".into());
        });

        let alert = rule.evaluate(&event, store.as_ref()).await;
        assert!(alert.is_none());
    }
}
