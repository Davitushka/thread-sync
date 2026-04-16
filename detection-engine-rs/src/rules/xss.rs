use std::collections::HashMap;

use chrono::Utc;
use regex::Regex;

use crate::alert::{Alert, AlertSeverity};
use crate::event::Event;

use super::Rule;

const XSS_SIGNATURES: &[&str] = &[
    r"(?i)<\s*script\b",
    r"(?i)javascript\s*:",
    r"(?i)vbscript\s*:",
    r"(?i)on(error|load|click|mouseover|focus|blur|submit|change)\s*=",
    r"(?i)<\s*img\b[^>]*\bon\w+\s*=",
    r"(?i)<\s*svg\b[^>]*\bon\w+\s*=",
    r"(?i)<\s*iframe\b",
    r"(?i)<\s*object\b",
    r"(?i)<\s*embed\b",
    r"(?i)document\.(cookie|location|write)",
    r"(?i)eval\s*\(",
    r"(?i)alert\s*\(",
    r"(?i)String\.fromCharCode",
    r"(?i)\\u[0-9a-fA-F]{4}.*<\s*script",
    r"(?i)%3[Cc]script",
    r"(?i)<\s*/\s*script\s*>",
];

const FP_PATTERNS: &[&str] = &[
    r"(?i)swagger",
    r"(?i)actuator/health",
    r"(?i)content-type.*text/html.*charset",
];

pub struct XssRule {
    patterns: Vec<Regex>,
    false_positives: Vec<Regex>,
}

impl XssRule {
    pub fn new() -> Self {
        Self {
            patterns: XSS_SIGNATURES
                .iter()
                .map(|s| Regex::new(s).unwrap())
                .collect(),
            false_positives: FP_PATTERNS.iter().map(|s| Regex::new(s).unwrap()).collect(),
        }
    }
}

impl Rule for XssRule {
    fn id(&self) -> &str {
        "xss_attempt"
    }

    fn title(&self) -> &str {
        "Cross-Site Scripting (XSS) Attempt Detected"
    }

    fn match_event(&self, event: &Event) -> Option<Alert> {
        let mut target = String::new();

        if !event.message.is_empty() {
            target.push_str(&event.message);
        }

        if let Some(ref path) = event.url_path {
            if !target.is_empty() {
                target.push(' ');
            }
            target.push_str(path);
        }

        if let Some(ref query) = event.metadata.get("QueryString") {
            if let Some(qs) = query.as_str() {
                target.push(' ');
                target.push_str(qs);
            }
        }

        if target.is_empty() {
            return None;
        }

        for fp in &self.false_positives {
            if fp.is_match(&target) {
                return None;
            }
        }

        let mut matched = Vec::new();
        for pat in &self.patterns {
            if pat.is_match(&target) {
                matched.push(pat.as_str().to_string());
            }
        }

        if matched.is_empty() {
            return None;
        }

        let severity = if event.status_code == Some(200) {
            AlertSeverity::Critical
        } else {
            AlertSeverity::High
        };

        let matched_short: Vec<String> = matched.iter().map(|m| truncate(m, 50)).collect();

        let mut context = HashMap::new();
        context.insert("matched_patterns".into(), serde_json::json!(matched_short));
        context.insert(
            "url_path".into(),
            serde_json::json!(Event::str_val(&event.url_path)),
        );
        context.insert(
            "http_method".into(),
            serde_json::json!(Event::str_val(&event.http_method)),
        );

        Some(Alert {
            rule_id: self.id().into(),
            rule_title: self.title().into(),
            severity,
            description: format!(
                "XSS attempt detected: {} pattern(s) matched from {} on {}",
                matched.len(),
                Event::str_val(&event.source_ip),
                Event::str_val(&event.url_path),
            ),
            source_ip: event.source_ip.clone(),
            user_id: event.user_id.clone(),
            event_ids: vec![event.event_id.clone()],
            mitre_tags: vec!["T1189".into(), "T1059.007".into()],
            fired_at: Utc::now(),
            context,
        })
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}...", &s[..n])
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::rules::test_utils::event_with;

    #[test]
    fn detects_script_tag_xss() {
        let rule = XssRule::new();
        let event = event_with(|e| {
            e.message = "Request: <script>alert('xss')</script>".into();
            e.url_path = Some("/api/search".into());
            e.status_code = Some(200);
        });

        let alert = rule.match_event(&event).expect("expected XSS alert");
        assert_eq!(alert.rule_id, "xss_attempt");
        assert_eq!(alert.severity, AlertSeverity::Critical);
    }

    #[test]
    fn detects_event_handler_xss() {
        let rule = XssRule::new();
        let event = event_with(|e| {
            e.message = "input: <img src=x onerror=alert(1)>".into();
        });

        let alert = rule.match_event(&event).expect("expected XSS alert");
        assert_eq!(alert.rule_id, "xss_attempt");
    }

    #[test]
    fn skips_normal_request() {
        let rule = XssRule::new();
        let event = event_with(|e| {
            e.message = "User searched for products".into();
            e.url_path = Some("/api/products?q=laptop".into());
        });

        let alert = rule.match_event(&event);
        assert!(alert.is_none());
    }
}
