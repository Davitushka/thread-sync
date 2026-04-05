use std::collections::HashMap;
use std::time::Duration;

use async_trait::async_trait;
use chrono::Utc;

use crate::alert::{Alert, AlertSeverity};
use crate::event::Event;
use crate::state_store::StateStore;

use super::{Rule, StatefulRule};

const ADMIN_PATHS: &[&str] = &[
    "/api/admin",
    "/api/internal",
    "/api/management",
    "/admin",
    "/manage",
    "/actuator",
    "/api/users/roles",
    "/api/permissions",
    "/api/audit",
];

pub struct PrivilegeEscalationRule {
    pub threshold: i64,
}

impl PrivilegeEscalationRule {
    pub fn new() -> Self {
        Self { threshold: 3 }
    }

    fn is_admin_path(path: &str) -> bool {
        let lower = path.to_lowercase();
        ADMIN_PATHS
            .iter()
            .any(|p| lower.starts_with(&p.to_lowercase()))
    }

    fn build_alert(
        &self,
        event: &Event,
        subtype: &str,
        description: String,
        severity: AlertSeverity,
    ) -> Alert {
        let mut context = HashMap::new();
        context.insert("subtype".into(), serde_json::json!(subtype));
        context.insert(
            "url_path".into(),
            serde_json::json!(Event::str_val(&event.url_path)),
        );
        context.insert(
            "user_role".into(),
            event
                .metadata
                .get("UserRole")
                .cloned()
                .unwrap_or(serde_json::Value::Null),
        );
        context.insert(
            "http_method".into(),
            serde_json::json!(Event::str_val(&event.http_method)),
        );

        Alert {
            rule_id: self.id().into(),
            rule_title: self.title().into(),
            severity,
            description,
            source_ip: event.source_ip.clone(),
            user_id: event.user_id.clone(),
            event_ids: vec![event.event_id.clone()],
            mitre_tags: vec!["T1068".into(), "T1078.003".into(), "T1548".into()],
            fired_at: Utc::now(),
            context,
        }
    }
}

impl Rule for PrivilegeEscalationRule {
    fn id(&self) -> &str {
        "privilege_escalation_attempt"
    }
    fn title(&self) -> &str {
        "Privilege Escalation or Unauthorized Admin Access Attempt"
    }

    fn match_event(&self, event: &Event) -> Option<Alert> {
        let url_path = event.url_path.as_ref()?;
        let code = event.status_code?;

        if event.event_type != "application" && event.event_type != "auth" {
            return None;
        }

        if !Self::is_admin_path(url_path) {
            return None;
        }

        if code == 403 {
            return Some(self.build_alert(
                event,
                "unauthorized_access",
                format!("Access denied (403) to admin endpoint: {}", url_path),
                AlertSeverity::High,
            ));
        }

        if (200..300).contains(&code) {
            if let Some(role) = event.metadata.get("UserRole").and_then(|v| v.as_str()) {
                if !role.is_empty() && role != "admin" && role != "superadmin" {
                    return Some(self.build_alert(
                        event,
                        "role_bypass",
                        format!(
                            "Non-admin user (role={}) accessed admin endpoint: {}",
                            role, url_path
                        ),
                        AlertSeverity::Critical,
                    ));
                }
            }
        }

        if (200..300).contains(&code) || code == 400 {
            let method = Event::str_val(&event.http_method);
            if (method == "PUT" || method == "PATCH" || method == "POST")
                && (url_path.contains("/roles") || url_path.contains("/permissions"))
            {
                return Some(self.build_alert(
                    event,
                    "role_modification",
                    format!(
                        "Role/permission modification attempt: {} {} (status={})",
                        method, url_path, code
                    ),
                    AlertSeverity::Critical,
                ));
            }
        }

        None
    }
}

#[async_trait]
impl StatefulRule for PrivilegeEscalationRule {
    async fn evaluate(&self, event: &Event, state: &dyn StateStore) -> Option<Alert> {
        let ip = event.source_ip.as_ref()?;
        let url_path = event.url_path.as_ref()?;

        if !Self::is_admin_path(url_path) {
            return None;
        }

        if event.status_code != Some(403) {
            return None;
        }

        let key = format!("priv:{}", ip);
        let count = state.increment(&key, Duration::from_secs(300)).await.ok()?;

        if count != self.threshold {
            return None;
        }

        let mut context = HashMap::new();
        context.insert("attempt_count".into(), serde_json::json!(count));
        context.insert("url_path".into(), serde_json::json!(url_path));
        context.insert("window".into(), serde_json::json!("5m"));

        Some(Alert {
            rule_id: self.id().into(),
            rule_title: self.title().into(),
            severity: AlertSeverity::Critical,
            description: format!(
                "Repeated privilege escalation attempts: {} forbidden requests to admin endpoints from {}",
                self.threshold, ip,
            ),
            source_ip: Some(ip.clone()),
            user_id: event.user_id.clone(),
            event_ids: vec![event.event_id.clone()],
            mitre_tags: vec!["T1068".into(), "T1078.003".into(), "T1548".into()],
            fired_at: Utc::now(),
            context,
        })
    }
}
