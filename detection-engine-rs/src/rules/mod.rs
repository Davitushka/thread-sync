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
