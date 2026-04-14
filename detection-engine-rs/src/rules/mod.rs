pub mod brute_force;
pub mod privilege_escalation;
pub mod rate_limit;
pub mod sql_injection;

use crate::alert::Alert;
use crate::event::Event;
use crate::state_store::StateStore;
use async_trait::async_trait;

pub trait Rule: Send + Sync {
    fn id(&self) -> &str;
    fn title(&self) -> &str;
    fn match_event(&self, event: &Event) -> Option<Alert>;
}

#[async_trait]
pub trait StatefulRule: Rule {
    async fn evaluate(&self, event: &Event, state: &dyn StateStore) -> Option<Alert>;
}

pub(crate) fn format_duration(d: std::time::Duration) -> String {
    let total_secs = d.as_secs();
    let mins = total_secs / 60;
    let secs = total_secs % 60;
    if mins > 0 && secs > 0 {
        format!("{}m{}s", mins, secs)
    } else if mins > 0 {
        format!("{}m0s", mins)
    } else {
        format!("{}s", secs)
    }
}

#[cfg(test)]
pub(crate) mod test_utils {
    use std::collections::{HashMap, HashSet};
    use std::sync::Mutex;
    use std::time::Duration;

    use anyhow::Result;
    use async_trait::async_trait;
    use chrono::Utc;
    use serde_json::Value;

    use crate::event::Event;
    use crate::state_store::StateStore;

    #[derive(Default)]
    pub struct MockStateStore {
        counters: Mutex<HashMap<String, i64>>,
        sets: Mutex<HashMap<String, HashSet<String>>>,
    }

    #[async_trait]
    impl StateStore for MockStateStore {
        async fn increment(&self, key: &str, _ttl: Duration) -> Result<i64> {
            let mut counters = self.counters.lock().expect("counter lock poisoned");
            let value = counters.entry(key.to_string()).or_insert(0);
            *value += 1;
            Ok(*value)
        }

        async fn get(&self, key: &str) -> Result<i64> {
            let counters = self.counters.lock().expect("counter lock poisoned");
            Ok(*counters.get(key).unwrap_or(&0))
        }

        async fn add_to_set(&self, key: &str, member: &str, _ttl: Duration) -> Result<i64> {
            let mut sets = self.sets.lock().expect("set lock poisoned");
            let entry = sets.entry(key.to_string()).or_default();
            Ok(if entry.insert(member.to_string()) {
                1
            } else {
                0
            })
        }

        async fn set_size(&self, key: &str) -> Result<i64> {
            let sets = self.sets.lock().expect("set lock poisoned");
            Ok(sets.get(key).map(|s| s.len() as i64).unwrap_or(0))
        }
    }

    pub fn event_with(overrides: impl FnOnce(&mut Event)) -> Event {
        let mut event = Event {
            timestamp: Utc::now(),
            event_id: "evt-1".into(),
            source_type: "dotnet".into(),
            event_type: "application".into(),
            severity: "warning".into(),
            message: "default message".into(),
            host: "svc-1".into(),
            source_ip: Some("10.0.0.1".into()),
            user_id: Some("u-1".into()),
            action: None,
            status_code: Some(200),
            url_path: Some("/api/orders".into()),
            http_method: Some("GET".into()),
            duration_ms: None,
            metadata: HashMap::new(),
        };
        overrides(&mut event);
        event
    }

    pub fn set_metadata(event: &mut Event, key: &str, value: Value) {
        event.metadata.insert(key.to_string(), value);
    }
}
