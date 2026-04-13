use std::collections::HashMap;
use std::time::Duration;

use anyhow::{anyhow, Result};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::Value;

#[derive(Debug, Clone, Serialize)]
pub struct DetectionsOverview {
    pub stats: DetectionsStats,
    pub severity_breakdown: Vec<DetectionBreakdown>,
    pub state_breakdown: Vec<DetectionBreakdown>,
    pub top_rules: Vec<DetectionBreakdown>,
    pub firing_rows: Vec<DetectionRowSummary>,
    pub rules: Vec<RuleSummary>,
}

#[derive(Debug, Clone, Serialize)]
pub struct DetectionsStats {
    pub rules_count: u32,
    pub pending_alerts: u32,
    pub alert_capacity: u32,
    pub firing_count: u32,
    pub critical_firing: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct DetectionBreakdown {
    pub name: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct DetectionRowSummary {
    pub rule: String,
    pub severity: String,
    pub state: String,
    pub signal: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct RuleSummary {
    pub id: String,
    pub title: String,
    pub severity: String,
    pub kind: Option<String>,
    pub threshold: Option<u32>,
    pub window_sec: Option<u32>,
    pub firing_count: u32,
}

#[derive(Debug, Clone)]
pub struct DetectionsOverviewService {
    http: reqwest::Client,
    correlator: String,
    prometheus: String,
}

impl DetectionsOverviewService {
    pub fn new(http: reqwest::Client, correlator: String, prometheus: String) -> Self {
        Self {
            http,
            correlator,
            prometheus,
        }
    }

    pub async fn overview(&self, timeout: Duration) -> Result<DetectionsOverview> {
        let (stats, rules, firing_rows) = tokio::try_join!(
            self.fetch_stats(timeout),
            self.fetch_rules(timeout),
            self.fetch_firing_rows(timeout),
        )?;

        let mut severity_counts: HashMap<String, u32> = HashMap::new();
        let mut state_counts: HashMap<String, u32> = HashMap::new();
        let mut top_rule_counts: HashMap<String, u32> = HashMap::new();

        for row in &firing_rows {
            *severity_counts.entry(row.severity.clone()).or_default() += 1;
            *state_counts.entry(row.state.clone()).or_default() += 1;
            *top_rule_counts.entry(row.rule.clone()).or_default() += 1;
        }

        let critical_firing = firing_rows
            .iter()
            .filter(|row| row.severity.eq_ignore_ascii_case("critical"))
            .count() as u32;
        let firing_count = firing_rows.len() as u32;
        let top_rules_rows = sorted_counts(top_rule_counts.clone())
            .into_iter()
            .take(8)
            .collect::<Vec<_>>();

        let mut rules = rules
            .into_iter()
            .map(|rule| RuleSummary {
                firing_count: top_rule_counts
                    .get(&rule.title)
                    .copied()
                    .or_else(|| top_rule_counts.get(&rule.id).copied())
                    .unwrap_or_default(),
                ..rule
            })
            .collect::<Vec<_>>();
        rules.sort_by(|a, b| {
            b.firing_count
                .cmp(&a.firing_count)
                .then_with(|| severity_rank(&b.severity).cmp(&severity_rank(&a.severity)))
                .then_with(|| a.title.cmp(&b.title))
        });

        Ok(DetectionsOverview {
            stats: DetectionsStats {
                rules_count: stats.rules_count,
                pending_alerts: stats.pending_alerts,
                alert_capacity: stats.alert_capacity,
                firing_count,
                critical_firing,
            },
            severity_breakdown: sorted_counts(severity_counts),
            state_breakdown: sorted_counts(state_counts),
            top_rules: top_rules_rows,
            firing_rows,
            rules,
        })
    }

    async fn fetch_stats(&self, timeout: Duration) -> Result<UpstreamStats> {
        let base: Url = self.correlator.parse()?;
        let url = base.join("/api/v1/stats")?;
        let resp = self.http.get(url).timeout(timeout).send().await?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            return Err(anyhow!("correlator stats responded {}: {}", status, body));
        }
        Ok(serde_json::from_str::<UpstreamStats>(&body)?)
    }

    async fn fetch_rules(&self, timeout: Duration) -> Result<Vec<RuleSummary>> {
        let base: Url = self.correlator.parse()?;
        let url = base.join("/api/v1/rules")?;
        let resp = self.http.get(url).timeout(timeout).send().await?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            return Err(anyhow!("correlator rules responded {}: {}", status, body));
        }

        let payload = serde_json::from_str::<Value>(&body)?;
        let rows = if let Some(arr) = payload.as_array() {
            arr.clone()
        } else {
            payload
                .get("rules")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default()
        };

        Ok(rows
            .into_iter()
            .map(|row| RuleSummary {
                id: get_str(&row, "id").unwrap_or_else(|| "rule".to_string()),
                title: get_str(&row, "title")
                    .or_else(|| get_str(&row, "id"))
                    .unwrap_or_else(|| "Rule".to_string()),
                severity: get_str(&row, "severity").unwrap_or_else(|| "unknown".to_string()),
                kind: get_str(&row, "kind"),
                threshold: get_u32(&row, "threshold"),
                window_sec: get_u32(&row, "window_sec"),
                firing_count: 0,
            })
            .collect())
    }

    async fn fetch_firing_rows(&self, timeout: Duration) -> Result<Vec<DetectionRowSummary>> {
        let base: Url = self.prometheus.parse()?;
        let mut url = base.join("/api/v1/query")?;
        url.query_pairs_mut().append_pair("query", "ALERTS");
        let resp = self.http.get(url).timeout(timeout).send().await?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            return Err(anyhow!("prometheus alerts responded {}: {}", status, body));
        }

        let payload = serde_json::from_str::<PromQueryResponse>(&body)?;
        Ok(payload
            .data
            .and_then(|data| data.result)
            .unwrap_or_default()
            .into_iter()
            .map(|item| DetectionRowSummary {
                rule: item
                    .metric
                    .get("alertname")
                    .cloned()
                    .unwrap_or_else(|| "alert".to_string()),
                severity: item
                    .metric
                    .get("severity")
                    .cloned()
                    .unwrap_or_else(|| "unknown".to_string()),
                state: item
                    .metric
                    .get("alertstate")
                    .cloned()
                    .unwrap_or_else(|| "firing".to_string()),
                signal: item.value.map(|pair| pair.1).unwrap_or_else(|| "0".to_string()),
            })
            .collect())
    }
}

#[derive(Debug, Deserialize)]
struct UpstreamStats {
    rules_count: u32,
    pending_alerts: u32,
    alert_capacity: u32,
}

#[derive(Debug, Deserialize)]
struct PromQueryResponse {
    data: Option<PromQueryData>,
}

#[derive(Debug, Deserialize)]
struct PromQueryData {
    result: Option<Vec<PromQueryItem>>,
}

#[derive(Debug, Deserialize)]
struct PromQueryItem {
    #[serde(default)]
    metric: HashMap<String, String>,
    value: Option<(f64, String)>,
}

fn sorted_counts(input: HashMap<String, u32>) -> Vec<DetectionBreakdown> {
    let mut rows = input
        .into_iter()
        .map(|(name, count)| DetectionBreakdown { name, count })
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| severity_rank(&b.name).cmp(&severity_rank(&a.name)))
            .then_with(|| a.name.cmp(&b.name))
    });
    rows
}

fn severity_rank(value: &str) -> u8 {
    match value {
        "critical" => 7,
        "high" | "error" => 6,
        "warning" | "medium" => 5,
        "info" | "low" => 4,
        "debug" => 3,
        _ => 1,
    }
}

fn get_str(value: &Value, key: &str) -> Option<String> {
    value
        .get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|value| !value.is_empty())
        .map(ToString::to_string)
}

fn get_u32(value: &Value, key: &str) -> Option<u32> {
    value
        .get(key)
        .and_then(Value::as_u64)
        .and_then(|value| u32::try_from(value).ok())
}
