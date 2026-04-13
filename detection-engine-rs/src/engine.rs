use std::sync::Arc;

use tokio::sync::{mpsc, RwLock};
use tracing::{info, warn};

use crate::alert::Alert;
use crate::event::Event;
use crate::metrics::EngineMetrics;
use crate::rules::{Rule, StatefulRule};
use crate::state_store::StateStore;

pub struct Engine {
    stateless_rules: RwLock<Vec<Box<dyn Rule>>>,
    stateful_rules: RwLock<Vec<Box<dyn StatefulRule>>>,
    state: Option<Arc<dyn StateStore>>,
    alert_tx: mpsc::Sender<Alert>,
    metrics: EngineMetrics,
}

impl Engine {
    pub fn new(
        stateless: Vec<Box<dyn Rule>>,
        stateful: Vec<Box<dyn StatefulRule>>,
        state: Option<Arc<dyn StateStore>>,
        alert_tx: mpsc::Sender<Alert>,
    ) -> Self {
        Self {
            stateless_rules: RwLock::new(stateless),
            stateful_rules: RwLock::new(stateful),
            state,
            alert_tx,
            metrics: EngineMetrics::new(),
        }
    }

    pub async fn process_raw(&self, payload: &[u8]) {
        let event: Event = match serde_json::from_slice(payload) {
            Ok(e) => e,
            Err(err) => {
                let preview_len = payload.len().min(200);
                let preview =
                    std::str::from_utf8(&payload[..preview_len]).unwrap_or("<binary>");
                warn!(%err, payload = preview, "failed to deserialize event");
                self.metrics.parse_errors.inc();
                return;
            }
        };
        self.process(&event).await;
    }

    pub async fn process(&self, event: &Event) {
        let start = std::time::Instant::now();
        self.metrics.events_processed.inc();

        {
            let rules = self.stateless_rules.read().await;
            for rule in rules.iter() {
                if let Some(alert) = rule.match_event(event) {
                    self.emit_alert(alert, rule.id()).await;
                }
            }
        }

        {
            let rules = self.stateful_rules.read().await;
            for rule in rules.iter() {
                if let Some(ref state) = self.state {
                    if let Some(alert) = rule.evaluate(event, state.as_ref()).await {
                        self.emit_alert(alert, rule.id()).await;
                    }
                } else if let Some(alert) = rule.match_event(event) {
                    self.emit_alert(alert, rule.id()).await;
                }
            }
        }

        self.metrics
            .process_duration
            .observe(start.elapsed().as_secs_f64());
    }

    pub async fn add_rule(&self, rule: Box<dyn Rule>) {
        let mut rules = self.stateless_rules.write().await;
        rules.push(rule);
    }

    pub async fn rule_count(&self) -> usize {
        let stateless = self.stateless_rules.read().await;
        let stateful = self.stateful_rules.read().await;
        stateless.len() + stateful.len()
    }

    async fn emit_alert(&self, alert: Alert, rule_id: &str) {
        let severity = alert.severity.to_string();
        self.metrics
            .alerts_fired
            .with_label_values(&[&severity, &rule_id.to_string()])
            .inc();

        info!(
            rule_id = alert.rule_id,
            severity = %alert.severity,
            description = alert.description,
            "alert fired",
        );

        if self.alert_tx.try_send(alert).is_err() {
            warn!(rule_id, "alert channel full, dropping alert");
            self.metrics.alerts_dropped.inc();
        }
    }
}
