use std::sync::Arc;

use parking_lot::RwLock as PlRwLock;
use tokio::sync::{mpsc, RwLock};
use tracing::{info, warn};

use crate::alert::Alert;
use crate::event::Event;
use crate::metrics::EngineMetrics;
use crate::rules::{Rule, StatefulRule};
use crate::state_store::StateStore;

pub struct Engine {
    /// Stateless rules use parking_lot::RwLock — no .await in match_event,
    /// so the lock is never held across yield points. parking_lot is faster
    /// for read-heavy workloads (no async overhead).
    stateless_rules: PlRwLock<Vec<Box<dyn Rule>>>,
    /// Stateful rules need tokio::sync::RwLock because evaluate() is async
    /// and the read guard must be held across .await points.
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
            stateless_rules: PlRwLock::new(stateless),
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

        // Stateless: parking_lot read lock, no .await needed, collect alerts then emit
        let stateless_alerts: Vec<(Alert, String)> = {
            let rules = self.stateless_rules.read();
            rules
                .iter()
                .filter_map(|rule| {
                    rule.match_event(event).map(|alert| (alert, rule.id().to_string()))
                })
                .collect()
        };
        for (alert, rule_id) in stateless_alerts {
            self.emit_alert(alert, &rule_id).await;
        }

        // Stateful: tokio::sync::RwLock, held across .await in evaluate()
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
        let mut rules = self.stateless_rules.write();
        rules.push(rule);
    }

    pub async fn rule_count(&self) -> usize {
        let stateless = self.stateless_rules.read();
        let stateful = self.stateful_rules.read().await;
        stateless.len() + stateful.len()
    }

    async fn emit_alert(&self, alert: Alert, rule_id: &str) {
        let severity = alert.severity.as_str();
        self.metrics
            .alerts_fired
            .with_label_values(&[severity, rule_id])
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
    }
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

        // Collect matching alerts outside the lock to avoid holding it across .await
        let stateless_alerts: Vec<(Alert, String)> = {
            let rules = self.stateless_rules.read();
            rules
                .iter()
                .filter_map(|rule| {
                    rule.match_event(event).map(|alert| (alert, rule.id().to_string()))
                })
                .collect()
        };
        for (alert, rule_id) in stateless_alerts {
            self.emit_alert(alert, &rule_id).await;
        }

        // Stateful rules: snapshot rule IDs, then evaluate outside the lock
        let stateful_snapshot: Vec<String> = {
            let rules = self.stateful_rules.read();
            rules.iter().map(|r| r.id().to_string()).collect()
        };

        for rule_id in stateful_snapshot {
            let rule_guard = self.stateful_rules.read();
            if let Some(rule) = rule_guard.iter().find(|r| r.id() == rule_id) {
                let alert_opt = if self.state.is_some() {
                    // Must drop lock before async evaluate
                    // Clone is cheap for stateless evaluation; for stateful we need to release
                    drop(rule_guard);
                    let rule_guard2 = self.stateful_rules.read();
                    let rule2 = rule_guard2.iter().find(|r| r.id() == rule_id);
                    if let Some(rule2) = rule2 {
                        if let Some(ref state) = self.state {
                            rule2.evaluate(event, state.as_ref()).await
                        } else {
                            rule2.match_event(event)
                        }
                    } else {
                        None
                    }
                } else {
                    rule.match_event(event)
                };
                drop(rule_guard);
                if let Some(alert) = alert_opt {
                    self.emit_alert(alert, &rule_id).await;
                }
            } else {
                drop(rule_guard);
            }
        }

        self.metrics
            .process_duration
            .observe(start.elapsed().as_secs_f64());
    }

    pub async fn add_rule(&self, rule: Box<dyn Rule>) {
        let mut rules = self.stateless_rules.write();
        rules.push(rule);
    }

    pub async fn rule_count(&self) -> usize {
        let stateless = self.stateless_rules.read();
        let stateful = self.stateful_rules.read();
        stateless.len() + stateful.len()
    }

    async fn emit_alert(&self, alert: Alert, rule_id: &str) {
        let severity = alert.severity.as_str();
        self.metrics
            .alerts_fired
            .with_label_values(&[severity, rule_id])
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
    }
}
