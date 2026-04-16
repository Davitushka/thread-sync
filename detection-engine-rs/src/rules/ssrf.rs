use std::collections::HashMap;

use chrono::Utc;

use crate::alert::{Alert, AlertSeverity};
use crate::event::Event;

use super::Rule;

const SSRF_PRIVATE_RANGES: &[&str] = &[
    "127.0.0.",
    "10.",
    "172.16.",
    "172.17.",
    "172.18.",
    "172.19.",
    "172.20.",
    "172.21.",
    "172.22.",
    "172.23.",
    "172.24.",
    "172.25.",
    "172.26.",
    "172.27.",
    "172.28.",
    "172.29.",
    "172.30.",
    "172.31.",
    "192.168.",
    "169.254.",
    "::1",
    "0.0.0.0",
    "localhost",
];

const SSRF_SENSITIVE_PATHS: &[&str] = &[
    "/metadata",
    "/latest/meta-data",
    "/latest/user-data",
    "/internal/",
    "/admin/config",
    "/debug/",
    "/env",
    "/actuator/env",
    "/.env",
    "/server-status",
    "/server-info",
];

pub struct SsrfRule;

impl SsrfRule {
    pub fn new() -> Self {
        Self
    }

    fn is_internal_target(ip: &str) -> bool {
        SSRF_PRIVATE_RANGES
            .iter()
            .any(|prefix| ip.starts_with(prefix))
    }

    fn is_ssrf_path(path: &str) -> bool {
        let lower = path.to_lowercase();
        SSRF_SENSITIVE_PATHS
            .iter()
            .any(|p| lower.contains(&p.to_lowercase()))
    }

    fn extract_target_from_url(url: &str) -> Option<String> {
        // Try to extract host/IP from URL patterns like http://10.0.0.1/...
        if let Some(start) = url.find("://") {
            let after_scheme = &url[start + 3..];
            let host_part = after_scheme.split('/').next().unwrap_or("");
            // Strip port
            let host = host_part.split(':').next().unwrap_or(host_part);
            return Some(host.to_string());
        }
        None
    }
}

impl Rule for SsrfRule {
    fn id(&self) -> &str {
        "ssrf_attempt"
    }

    fn title(&self) -> &str {
        "Server-Side Request Forgery (SSRF) Attempt Detected"
    }

    fn match_event(&self, event: &Event) -> Option<Alert> {
        let path = event.url_path.as_ref()?;
        let source_ip = event.source_ip.as_ref()?;

        let mut target_internal = false;
        let mut target_host = String::new();

        // Check if the URL path targets internal resources
        if let Some(host) = Self::extract_target_from_url(path) {
            target_host = host.clone();
            if Self::is_internal_target(&host) {
                target_internal = true;
            }
        }

        // Check metadata for target URL (common in SSRF payloads)
        if !target_internal {
            if let Some(url_val) = event
                .metadata
                .get("TargetUrl")
                .or_else(|| event.metadata.get("url"))
            {
                if let Some(url_str) = url_val.as_str() {
                    if let Some(host) = Self::extract_target_from_url(url_str) {
                        target_host = host.clone();
                        if Self::is_internal_target(&host) {
                            target_internal = true;
                        }
                    }
                }
            }
        }

        // Check if path itself contains SSRF-sensitive endpoints
        let sensitive_path = Self::is_ssrf_path(path);

        // Alert conditions: internal target OR sensitive path from external source
        if !target_internal && !sensitive_path {
            return None;
        }

        // Only alert if the source is external (not from internal IPs themselves)
        // For now, alert on any SSRF pattern; tuning can add source IP whitelisting
        let severity = if target_internal && event.status_code == Some(200) {
            AlertSeverity::Critical
        } else if target_internal {
            AlertSeverity::High
        } else {
            AlertSeverity::Medium
        };

        let mut context = HashMap::new();
        context.insert("target_internal".into(), serde_json::json!(target_internal));
        if !target_host.is_empty() {
            context.insert("target_host".into(), serde_json::json!(target_host));
        }
        context.insert("sensitive_path".into(), serde_json::json!(sensitive_path));
        context.insert(
            "url_path".into(),
            serde_json::json!(Event::str_val(&event.url_path)),
        );
        context.insert(
            "status_code".into(),
            serde_json::json!(Event::int_val(&event.status_code)),
        );

        Some(Alert {
            rule_id: self.id().into(),
            rule_title: self.title().into(),
            severity,
            description: format!(
                "SSRF attempt from {} targeting {}{}",
                source_ip,
                if target_internal {
                    "internal resource"
                } else {
                    "sensitive endpoint"
                },
                if !target_host.is_empty() {
                    format!(" ({})", target_host)
                } else {
                    String::new()
                },
            ),
            source_ip: Some(source_ip.clone()),
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
    fn detects_ssrf_to_internal_ip() {
        let rule = SsrfRule::new();
        let event = event_with(|e| {
            e.url_path = Some("/api/fetch?url=http://10.0.0.1/admin".into());
            e.source_ip = Some("203.0.113.5".into());
            e.status_code = Some(200);
        });

        let alert = rule.match_event(&event).expect("expected SSRF alert");
        assert_eq!(alert.rule_id, "ssrf_attempt");
        assert_eq!(alert.severity, AlertSeverity::Critical);
    }

    #[test]
    fn detects_ssrf_to_metadata_endpoint() {
        let rule = SsrfRule::new();
        let event = event_with(|e| {
            e.url_path = Some("/api/proxy?dest=http://169.254.169.254/latest/meta-data".into());
            e.source_ip = Some("203.0.113.5".into());
        });

        let alert = rule.match_event(&event).expect("expected SSRF alert");
        assert_eq!(alert.rule_id, "ssrf_attempt");
    }

    #[test]
    fn skips_normal_external_request() {
        let rule = SsrfRule::new();
        let event = event_with(|e| {
            e.url_path = Some("/api/users/profile".into());
            e.source_ip = Some("10.0.0.50".into());
        });

        let alert = rule.match_event(&event);
        assert!(alert.is_none());
    }
}
