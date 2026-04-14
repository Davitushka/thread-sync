use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;

use crate::alert::{Alert, AlertSeverity};
use crate::event::Event;
use crate::state_store::StateStore;

use super::{Rule, StatefulRule, format_duration};

const KNOWN_BOTS: &[&str] = &[
    "googlebot",
    "bingbot",
    "health-check",
    "uptime-robot",
    "pingdom",
    "datadog",
];

fn is_known_bot(user_agent: &str) -> bool {
    let ua = user_agent.to_lowercase();
    KNOWN_BOTS.iter().any(|bot| ua.contains(bot))
}

pub struct RateLimitEvasionRule {
    pub threshold: i64,
    pub window: Duration,
}

impl RateLimitEvasionRule {
    pub fn new() -> Self {
        Self {
            threshold: 500,
            window: Duration::from_secs(60),
        }
    }
}

impl Rule for RateLimitEvasionRule {
    fn id(&self) -> &str {
        "rate_limit_evasion"
    }
    fn title(&self) -> &str {
        "Rate Limit Evasion \u{2014} Anomalous Request Volume from Single IP"
    }
    fn match_event(&self, _event: &Event) -> Option<Alert> {
        None
    }
}

#[async_trait]
impl StatefulRule for RateLimitEvasionRule {
    async fn evaluate(&self, event: &Event, state: &dyn StateStore) -> Option<Alert> {
        let ip = event.source_ip.as_ref()?;

        if event.source_type != "dotnet" && event.source_type != "nginx" {
            return None;
        }

        if let Some(ua) = event.metadata.get("UserAgent").and_then(|v| v.as_str()) {
            if is_known_bot(ua) {
                return None;
            }
        }

        let key = format!("rle:{}", ip);
        let count = state.increment(&key, self.window).await.ok()?;

        if count != self.threshold {
            return None;
        }

        let mut context = HashMap::new();
        context.insert("request_count".into(), serde_json::json!(count));
        context.insert(
            "window".into(),
            serde_json::json!(format_duration(self.window)),
        );
        context.insert("threshold".into(), serde_json::json!(self.threshold));

        Some(Alert {
            rule_id: self.id().into(),
            rule_title: self.title().into(),
            severity: AlertSeverity::Medium,
            description: format!(
                "High request volume: {} requests in {} from {} (possible rate limit bypass)",
                self.threshold,
                format_duration(self.window),
                ip,
            ),
            source_ip: Some(ip.clone()),
            user_id: None,
            event_ids: vec![event.event_id.clone()],
            mitre_tags: vec!["T1595".into(), "T1595.002".into(), "T1046".into()],
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
    async fn fires_on_threshold_for_supported_source() {
        let rule = RateLimitEvasionRule {
            threshold: 2,
            window: Duration::from_secs(60),
        };
        let store = Arc::new(MockStateStore::default());
        let event = event_with(|e| {
            e.source_type = "dotnet".into();
            e.source_ip = Some("10.10.10.10".into());
        });

        assert!(rule.evaluate(&event, store.as_ref()).await.is_none());
        let alert = rule
            .evaluate(&event, store.as_ref())
            .await
            .expect("expected alert");
        assert_eq!(alert.rule_id, "rate_limit_evasion");
        assert_eq!(alert.severity, AlertSeverity::Medium);
    }

    #[tokio::test]
    async fn ignores_known_bots() {
        let rule = RateLimitEvasionRule {
            threshold: 1,
            window: Duration::from_secs(60),
        };
        let store = Arc::new(MockStateStore::default());
        let event = event_with(|e| {
            e.source_type = "dotnet".into();
            set_metadata(e, "UserAgent", json!("GoogleBot/2.1"));
        });

        let alert = rule.evaluate(&event, store.as_ref()).await;
        assert!(alert.is_none());
    }
}
