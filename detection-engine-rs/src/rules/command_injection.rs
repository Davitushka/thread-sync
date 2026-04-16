use std::collections::HashMap;

use chrono::Utc;
use regex::Regex;

use crate::alert::{Alert, AlertSeverity};
use crate::event::Event;

use super::Rule;

const CMD_SIGNATURES: &[&str] = &[
    r"(?i)[;&|`]\s*(rm\s+-rf|chmod|chown|wget|curl)\b",
    r"(?i)[;&|`]\s*(cat|head|tail|less|more)\s+/etc/(passwd|shadow|hosts)",
    r"(?i)[;&|`]\s*(bash|sh|cmd|powershell|pwsh)\b",
    r"(?i)[;&|`]\s*(nc|ncat|netcat)\s+",
    r"(?i)\$\(\s*(cat|wget|curl|bash|sh|id|whoami|uname)\b",
    r"(?i)\bexec\s*\(\s*(/bin/|/usr/bin/)",
    r#"(?i)\bsystem\s*\(\s*['"]/"#,
    r#"(?i)\bpopen\s*\(\s*['"]/"#,
    r"(?i)(;\s*|\|\s*)python[23]?\s+-c\s+",
    r"(?i)(;\s*|\|\s*)perl\s+-e\s+",
    r"(?i)(;\s*|\|\s*)ruby\s+-e\s+",
    r"(?i)\b(eval|exec)\s*\(\s*request\s*\.",
    r"(?i)/bin/(ba)?sh\s+-c\s",
];

const FP_PATTERNS: &[&str] = &[
    r"(?i)health.check",
    r"(?i)swagger",
    r"(?i)actuator/health",
    r"(?i)\bcmd\b.*\b(ping|tracert|ipconfig)\b.*monitor",
];

pub struct CommandInjectionRule {
    patterns: Vec<Regex>,
    false_positives: Vec<Regex>,
}

impl CommandInjectionRule {
    pub fn new() -> Self {
        Self {
            patterns: CMD_SIGNATURES
                .iter()
                .map(|s| Regex::new(s).unwrap())
                .collect(),
            false_positives: FP_PATTERNS.iter().map(|s| Regex::new(s).unwrap()).collect(),
        }
    }
}

impl Rule for CommandInjectionRule {
    fn id(&self) -> &str {
        "command_injection"
    }

    fn title(&self) -> &str {
        "Command Injection Attempt Detected"
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

        let severity = if event.status_code == Some(500) || event.status_code == Some(200) {
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
            "status_code".into(),
            serde_json::json!(Event::int_val(&event.status_code)),
        );

        Some(Alert {
            rule_id: self.id().into(),
            rule_title: self.title().into(),
            severity,
            description: format!(
                "Command injection attempt: {} pattern(s) matched from {}",
                matched.len(),
                Event::str_val(&event.source_ip),
            ),
            source_ip: event.source_ip.clone(),
            user_id: event.user_id.clone(),
            event_ids: vec![event.event_id.clone()],
            mitre_tags: vec!["T1190".into(), "T1059".into()],
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
    fn detects_semicolon_command_injection() {
        let rule = CommandInjectionRule::new();
        let event = event_with(|e| {
            e.source_type = "dotnet".into();
            e.message = "Request: ; cat /etc/passwd".into();
            e.url_path = Some("/api/search".into());
            e.status_code = Some(200);
        });

        let alert = rule
            .match_event(&event)
            .expect("expected command injection alert");
        assert_eq!(alert.rule_id, "command_injection");
        assert_eq!(alert.severity, AlertSeverity::Critical);
    }

    #[test]
    fn detects_subshell_injection() {
        let rule = CommandInjectionRule::new();
        let event = event_with(|e| {
            e.source_type = "dotnet".into();
            e.message = "input: $(cat /etc/hosts)".into();
        });

        let alert = rule.match_event(&event).expect("expected subshell alert");
        assert_eq!(alert.rule_id, "command_injection");
    }

    #[test]
    fn skips_false_positive() {
        let rule = CommandInjectionRule::new();
        let event = event_with(|e| {
            e.message = "health.check endpoint ping monitor".into();
        });

        let alert = rule.match_event(&event);
        assert!(alert.is_none());
    }
}
