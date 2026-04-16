use std::collections::HashMap;

use chrono::Utc;

use crate::alert::{Alert, AlertSeverity};
use crate::event::Event;

use super::Rule;

const EXPECTED_METHODS: &[&str] = &["GET", "POST", "HEAD", "OPTIONS"];

const SENSITIVE_PATHS: &[&str] = &[
    "/api/admin",
    "/api/users",
    "/api/config",
    "/api/internal",
    "/api/management",
    "/api/permissions",
    "/api/roles",
    "/api/secrets",
    "/api/keys",
    "/actuator",
];

pub struct UnusualHttpMethodsRule;

impl UnusualHttpMethodsRule {
    pub fn new() -> Self {
        Self
    }
}

impl Rule for UnusualHttpMethodsRule {
    fn id(&self) -> &str {
        "unusual_http_methods"
    }

    fn title(&self) -> &str {
        "Unusual HTTP Method on Sensitive Endpoint"
    }

    fn match_event(&self, event: &Event) -> Option<Alert> {
        let method = event.http_method.as_ref()?;
        let path = event.url_path.as_ref()?;

        // Skip common methods
        if EXPECTED_METHODS.iter().any(|m| m == method) {
            return None;
        }

        let path_lower = path.to_lowercase();
        let is_sensitive = SENSITIVE_PATHS
            .iter()
            .any(|p| path_lower.starts_with(&p.to_lowercase()));

        if !is_sensitive {
            return None;
        }

        let severity = if event.status_code == Some(200) || event.status_code == Some(201) {
            AlertSeverity::Critical
        } else {
            AlertSeverity::Medium
        };

        let mut context = HashMap::new();
        context.insert("http_method".into(), serde_json::json!(method));
        context.insert("url_path".into(), serde_json::json!(path));
        context.insert(
            "status_code".into(),
            serde_json::json!(Event::int_val(&event.status_code)),
        );

        Some(Alert {
            rule_id: self.id().into(),
            rule_title: self.title().into(),
            severity,
            description: format!(
                "Unusual HTTP method {} used on sensitive endpoint {} (status={})",
                method,
                path,
                Event::int_val(&event.status_code),
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
    use super::*;
    use crate::rules::test_utils::event_with;

    #[test]
    fn detects_delete_on_admin_endpoint() {
        let rule = UnusualHttpMethodsRule::new();
        let event = event_with(|e| {
            e.http_method = Some("DELETE".into());
            e.url_path = Some("/api/admin/users/5".into());
            e.status_code = Some(200);
        });

        let alert = rule
            .match_event(&event)
            .expect("expected unusual method alert");
        assert_eq!(alert.rule_id, "unusual_http_methods");
        assert_eq!(alert.severity, AlertSeverity::Critical);
    }

    #[test]
    fn detects_put_on_sensitive_path() {
        let rule = UnusualHttpMethodsRule::new();
        let event = event_with(|e| {
            e.http_method = Some("PUT".into());
            e.url_path = Some("/api/config/settings".into());
        });

        let alert = rule
            .match_event(&event)
            .expect("expected unusual method alert");
        assert_eq!(alert.rule_id, "unusual_http_methods");
    }

    #[test]
    fn skips_normal_get_request() {
        let rule = UnusualHttpMethodsRule::new();
        let event = event_with(|e| {
            e.http_method = Some("GET".into());
            e.url_path = Some("/api/admin/dashboard".into());
        });

        let alert = rule.match_event(&event);
        assert!(alert.is_none());
    }

    #[test]
    fn skips_unusual_method_on_non_sensitive_path() {
        let rule = UnusualHttpMethodsRule::new();
        let event = event_with(|e| {
            e.http_method = Some("DELETE".into());
            e.url_path = Some("/api/posts/123".into());
        });

        let alert = rule.match_event(&event);
        assert!(alert.is_none());
    }
}
