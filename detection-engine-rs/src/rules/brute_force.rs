use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;

use crate::alert::{Alert, AlertSeverity};
use crate::event::Event;
use crate::state_store::StateStore;

use super::{Rule, StatefulRule, format_duration};

const AUTH_PATHS: &[&str] = &[
    "/api/auth",
    "/api/login",
    "/api/token",
    "/hubs/",
    "/api/account",
];

pub struct BruteForceRule {
    pub threshold: i64,
    pub window: Duration,
}

impl BruteForceRule {
    pub fn new() -> Self {
        Self {
            threshold: 10,
            window: Duration::from_secs(120),
        }
    }

    fn is_candidate(&self, event: &Event) -> bool {
        match event.status_code {
            Some(401) | Some(403) => {}
            _ => return false,
        }
        let path = Event::str_val(&event.url_path);
        if !AUTH_PATHS.iter().any(|p| path.contains(p)) {
            return false;
        }
        event.source_ip.is_some()
    }
}

impl Rule for BruteForceRule {
    fn id(&self) -> &str {
        "brute_force_api"
    }
    fn title(&self) -> &str {
        "API / SignalR Brute-Force Authentication Attempts"
    }
    fn match_event(&self, _event: &Event) -> Option<Alert> {
        None
    }
}

#[async_trait]
impl StatefulRule for BruteForceRule {
    async fn evaluate(&self, event: &Event, state: &dyn StateStore) -> Option<Alert> {
        if !self.is_candidate(event) {
            return None;
        }

        let ip = event.source_ip.as_ref()?;
        let key = format!("bf:{}", ip);
        let count = state.increment(&key, self.window).await.ok()?;

        if count != self.threshold {
            return None;
        }

        let path = Event::str_val(&event.url_path);
        let mut context = HashMap::new();
        context.insert("failed_attempts".into(), serde_json::json!(count));
        context.insert(
            "window".into(),
            serde_json::json!(format_duration(self.window)),
        );
        context.insert("url_path".into(), serde_json::json!(path));

        Some(Alert {
            rule_id: self.id().into(),
            rule_title: self.title().into(),
            severity: AlertSeverity::High,
            description: format!(
                "Brute-force detected: {} failed authentication attempts in {} from {}",
                self.threshold,
                format_duration(self.window),
                ip,
            ),
            source_ip: Some(ip.clone()),
            user_id: event.user_id.clone(),
            event_ids: vec![event.event_id.clone()],
            mitre_tags: vec!["T1110".into(), "T1110.001".into()],
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
    async fn fires_when_threshold_is_reached_on_auth_failures() {
        let rule = BruteForceRule {
            threshold: 3,
            window: Duration::from_secs(120),
        };
        let store = Arc::new(MockStateStore::default());
        let event = event_with(|e| {
            e.status_code = Some(401);
            e.url_path = Some("/api/auth/login".into());
            e.source_ip = Some("192.168.0.10".into());
        });

        assert!(rule.evaluate(&event, store.as_ref()).await.is_none());
        assert!(rule.evaluate(&event, store.as_ref()).await.is_none());

        let alert = rule
            .evaluate(&event, store.as_ref())
            .await
            .expect("expected alert on threshold");
        assert_eq!(alert.rule_id, "brute_force_api");
        assert_eq!(alert.severity, AlertSeverity::High);
    }

    #[tokio::test]
    async fn ignores_non_auth_or_non_failure_events() {
        let rule = BruteForceRule::new();
        let store = Arc::new(MockStateStore::default());
        let event = event_with(|e| {
            e.status_code = Some(200);
            e.url_path = Some("/api/orders".into());
        });

        let alert = rule.evaluate(&event, store.as_ref()).await;
        assert!(alert.is_none());
    }
}
