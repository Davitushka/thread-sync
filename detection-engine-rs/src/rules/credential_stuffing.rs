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
    "/api/account",
    "/hubs/",
];

pub struct CredentialStuffingRule {
    pub threshold: i64,
    pub window: Duration,
}

impl CredentialStuffingRule {
    pub fn new() -> Self {
        Self {
            threshold: 5,
            window: Duration::from_secs(300),
        }
    }

    fn is_auth_failure(event: &Event) -> bool {
        matches!(event.status_code, Some(401))
            && AUTH_PATHS
                .iter()
                .any(|p| Event::str_val(&event.url_path).contains(p))
    }
}

impl Rule for CredentialStuffingRule {
    fn id(&self) -> &str {
        "credential_stuffing"
    }
    fn title(&self) -> &str {
        "Credential Stuffing — Multiple Failed Logins from Different IPs for Same Account"
    }
    fn match_event(&self, _event: &Event) -> Option<Alert> {
        None
    }
}

#[async_trait]
impl StatefulRule for CredentialStuffingRule {
    async fn evaluate(&self, event: &Event, state: &dyn StateStore) -> Option<Alert> {
        if !Self::is_auth_failure(event) {
            return None;
        }

        let user_id = event.user_id.as_ref()?;
        let ip = event.source_ip.as_ref()?;

        // Track unique source IPs per user account
        let key = format!("cs:{}", user_id);
        state.add_to_set(&key, ip, self.window).await.ok()?;
        let unique_ips = state.set_size(&key).await.ok()?;

        if unique_ips < self.threshold {
            return None;
        }

        // Anti-spam: fire only once per window per user
        let antispan_key = format!("cs:fired:{}", user_id);
        if state.get(&antispan_key).await.unwrap_or(0) > 0 {
            return None;
        }
        if let Err(e) = state.increment(&antispan_key, self.window).await {
            tracing::warn!(error = %e, "antispan increment failed — alert may re-fire");
        }

        let mut context = HashMap::new();
        context.insert("unique_ips".into(), serde_json::json!(unique_ips));
        context.insert(
            "window".into(),
            serde_json::json!(format_duration(self.window)),
        );
        context.insert("user_id".into(), serde_json::json!(user_id));
        context.insert(
            "url_path".into(),
            serde_json::json!(Event::str_val(&event.url_path)),
        );

        Some(Alert {
            rule_id: self.id().into(),
            rule_title: self.title().into(),
            severity: AlertSeverity::High,
            description: format!(
                "Credential stuffing detected: {} unique IPs attempted login as '{}' in {}",
                self.threshold,
                user_id,
                format_duration(self.window),
            ),
            source_ip: Some(ip.clone()),
            user_id: Some(user_id.clone()),
            event_ids: vec![event.event_id.clone()],
            mitre_tags: vec!["T1110.004".into(), "T1110".into()],
            fired_at: Utc::now(),
            context,
        })
    }
}

#[cfg(test)]
mod tests {
    use std::sync::Arc;

    use super::*;
    use crate::rules::test_utils::MockStateStore;

    fn make_event(ip: &str, user: &str) -> Event {
        let mut event = crate::rules::test_utils::event_with(|e| {
            e.status_code = Some(401);
            e.url_path = Some("/api/auth/login".into());
            e.source_ip = Some(ip.into());
            e.user_id = Some(user.into());
        });
        event.event_id = format!("evt-{}", ip);
        event
    }

    #[tokio::test]
    async fn fires_when_unique_ip_threshold_reached() {
        let rule = CredentialStuffingRule {
            threshold: 3,
            window: Duration::from_secs(300),
        };
        let store = Arc::new(MockStateStore::default());

        assert!(
            rule.evaluate(&make_event("10.0.0.1", "alice"), store.as_ref())
                .await
                .is_none()
        );
        assert!(
            rule.evaluate(&make_event("10.0.0.2", "alice"), store.as_ref())
                .await
                .is_none()
        );
        let alert = rule
            .evaluate(&make_event("10.0.0.3", "alice"), store.as_ref())
            .await
            .expect("expected credential stuffing alert");
        assert_eq!(alert.rule_id, "credential_stuffing");
        assert_eq!(alert.severity, AlertSeverity::High);
    }

    #[tokio::test]
    async fn ignores_non_auth_events() {
        let rule = CredentialStuffingRule::new();
        let store = Arc::new(MockStateStore::default());
        let event = crate::rules::test_utils::event_with(|e| {
            e.status_code = Some(200);
            e.url_path = Some("/api/products".into());
        });

        let alert = rule.evaluate(&event, store.as_ref()).await;
        assert!(alert.is_none());
    }
}
