use std::collections::HashMap;

use chrono::Utc;
use regex::Regex;

use crate::alert::{Alert, AlertSeverity};
use crate::event::Event;

use super::Rule;

const SQL_SIGNATURES: &[&str] = &[
    r"(?i)(union\s+select|union\s+all\s+select)",
    r#"(?i)('|")\s*(or|and)\s*('|")?\s*\d+\s*=\s*\d+"#,
    r"(?i);\s*(drop|alter|truncate|create)\s+(table|database)",
    r"(?i)exec(\s|\+)+(x?p_|sp_)\w+",
    r"(?i)information_schema\.(tables|columns)",
    r"(?i)(sleep|benchmark|waitfor\s+delay)\s*\(",
    r"(?i)0x[0-9a-fA-F]{4,}",
    r"(?i)\bconvert\s*\(\s*int\s*,",
    r"(?i)char\s*\(\s*\d+\s*\)",
];

const NOSQL_SIGNATURES: &[&str] = &[
    r"(?i)\$where\s*:",
    r#"(?i)\$gt\s*:\s*(0|null|"")"#,
    r"(?i)\$regex\s*:",
    r"(?i)\}\s*,\s*\{.*\$",
    r"(?i)/\*.*\*/",
];

const FP_PATTERNS: &[&str] = &[
    r"(?i)health.check",
    r"(?i)swagger",
    r"(?i)actuator",
];

pub struct SQLInjectionRule {
    sql_patterns: Vec<Regex>,
    nosql_patterns: Vec<Regex>,
    false_positives: Vec<Regex>,
}

impl SQLInjectionRule {
    pub fn new() -> Self {
        Self {
            sql_patterns: SQL_SIGNATURES
                .iter()
                .map(|s| Regex::new(s).unwrap())
                .collect(),
            nosql_patterns: NOSQL_SIGNATURES
                .iter()
                .map(|s| Regex::new(s).unwrap())
                .collect(),
            false_positives: FP_PATTERNS
                .iter()
                .map(|s| Regex::new(s).unwrap())
                .collect(),
        }
    }
}

fn truncate(s: &str, n: usize) -> String {
    if s.len() <= n {
        s.to_string()
    } else {
        format!("{}...", &s[..n])
    }
}

impl Rule for SQLInjectionRule {
    fn id(&self) -> &str {
        "sql_injection_attempt"
    }

    fn title(&self) -> &str {
        "SQL/NoSQL Injection Attempt Detected in Application Logs"
    }

    fn match_event(&self, event: &Event) -> Option<Alert> {
        if event.source_type != "dotnet" && event.source_type != "postgresql" {
            return None;
        }

        let mut target = event.message.clone();
        if let Some(ref path) = event.url_path {
            target.push(' ');
            target.push_str(path);
        }

        for fp in &self.false_positives {
            if fp.is_match(&target) {
                return None;
            }
        }

        let mut matched = Vec::new();
        for pat in &self.sql_patterns {
            if pat.is_match(&target) {
                matched.push(pat.as_str().to_string());
            }
        }
        for pat in &self.nosql_patterns {
            if pat.is_match(&target) {
                matched.push(format!("nosql:{}", pat.as_str()));
            }
        }

        if matched.is_empty() {
            return None;
        }

        let severity = if event.source_type == "postgresql"
            || event.status_code == Some(500)
        {
            AlertSeverity::Critical
        } else {
            AlertSeverity::High
        };

        let matched_short: Vec<String> = matched.iter().map(|m| truncate(m, 40)).collect();

        let mut context = HashMap::new();
        context.insert(
            "matched_patterns".into(),
            serde_json::json!(matched_short),
        );
        context.insert(
            "source_type".into(),
            serde_json::json!(&event.source_type),
        );
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
                "SQL/NoSQL injection attempt detected: {} pattern(s) matched in {} event",
                matched.len(),
                event.source_type,
            ),
            source_ip: event.source_ip.clone(),
            user_id: event.user_id.clone(),
            event_ids: vec![event.event_id.clone()],
            mitre_tags: vec!["T1190".into(), "T1059.007".into()],
            fired_at: Utc::now(),
            context,
        })
    }
}
