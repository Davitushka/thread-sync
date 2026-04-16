use std::collections::HashMap;

use chrono::Utc;
use regex::Regex;

use crate::alert::{Alert, AlertSeverity};
use crate::event::Event;

use super::Rule;

const PATH_TRAVERSAL_SIGNATURES: &[&str] = &[
    r"\.\./\.\./",
    r"\.\.\\",
    r"%2e%2e[/%5c]",
    r"%2e%2e%2f",
    r"\.\.%2f",
    r"\.\.%5c",
    r"/etc/(passwd|shadow|hosts|hostname|group)",
    r"/proc/(self|version|cpuinfo|meminfo)",
    r"\\windows\\system32",
    r"\\winnt\\system32",
    r"\bboot\.ini\b",
    r"\bweb\.config\b",
    r"\.\.%252f",
    r"\.\.%c0%af",
    r"/var/log/(auth|syslog|secure)",
];

const FP_PATTERNS: &[&str] = &[
    r"(?i)swagger",
    r"(?i)actuator/health",
    r"(?i)node_modules",
    r"(?i)\.\./src/", // legitimate source code references
];

pub struct PathTraversalRule {
    patterns: Vec<Regex>,
    false_positives: Vec<Regex>,
}

impl PathTraversalRule {
    pub fn new() -> Self {
        Self {
            patterns: PATH_TRAVERSAL_SIGNATURES
                .iter()
                .map(|s| Regex::new(s).unwrap())
                .collect(),
            false_positives: FP_PATTERNS.iter().map(|s| Regex::new(s).unwrap()).collect(),
        }
    }
}

impl Rule for PathTraversalRule {
    fn id(&self) -> &str {
        "path_traversal"
    }

    fn title(&self) -> &str {
        "Path Traversal / Directory Traversal Attempt Detected"
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
        } else if event.status_code == Some(500) {
            AlertSeverity::High
        } else {
            AlertSeverity::Medium
        };

        let mut context = HashMap::new();
        context.insert("matched_count".into(), serde_json::json!(matched.len()));
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
                "Path traversal attempt from {} on {} ({} pattern(s))",
                Event::str_val(&event.source_ip),
                Event::str_val(&event.url_path),
                matched.len(),
            ),
            source_ip: event.source_ip.clone(),
            user_id: event.user_id.clone(),
            event_ids: vec![event.event_id.clone()],
            mitre_tags: vec!["T1083".into(), "T1190".into()],
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
    fn detects_classical_path_traversal() {
        let rule = PathTraversalRule::new();
        let event = event_with(|e| {
            e.message = "File access: ../../etc/passwd".into();
            e.url_path = Some("/api/files?path=../../etc/passwd".into());
            e.status_code = Some(200);
        });

        let alert = rule
            .match_event(&event)
            .expect("expected path traversal alert");
        assert_eq!(alert.rule_id, "path_traversal");
        assert_eq!(alert.severity, AlertSeverity::Critical);
    }

    #[test]
    fn detects_encoded_path_traversal() {
        let rule = PathTraversalRule::new();
        let event = event_with(|e| {
            e.url_path = Some("/api/download?file=%2e%2e%2f%2e%2e%2fetc/passwd".into());
        });

        let alert = rule
            .match_event(&event)
            .expect("expected encoded traversal alert");
        assert_eq!(alert.rule_id, "path_traversal");
    }

    #[test]
    fn skips_normal_file_access() {
        let rule = PathTraversalRule::new();
        let event = event_with(|e| {
            e.url_path = Some("/api/documents/report.pdf".into());
            e.status_code = Some(200);
        });

        let alert = rule.match_event(&event);
        assert!(alert.is_none());
    }
}
