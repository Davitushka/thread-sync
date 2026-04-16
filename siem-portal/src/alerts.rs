use std::collections::{BTreeMap, HashMap, HashSet};
use std::time::Duration;

use anyhow::{anyhow, Result};
use reqwest::Url;
use serde::{Deserialize, Serialize};

use crate::query_helpers::severity_rank;

#[derive(Debug, Clone, Serialize)]
pub struct AlertsOverview {
    pub totals: AlertTotals,
    pub severity_breakdown: Vec<AlertBreakdown>,
    pub source_breakdown: Vec<AlertBreakdown>,
    pub alerts: Vec<AlertRecord>,
}

#[derive(Debug, Clone, Serialize)]
pub struct AlertTotals {
    pub total: u32,
    pub active: u32,
    pub critical: u32,
    pub silenced: u32,
    pub unique_sources: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct AlertBreakdown {
    pub name: String,
    pub count: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct AlertRecord {
    pub fingerprint: String,
    pub name: String,
    pub severity: String,
    pub state: String,
    pub source: String,
    pub summary: String,
    pub description: String,
    pub starts_at: Option<String>,
    pub ends_at: Option<String>,
    pub rule_id: Option<String>,
    pub source_ip: Option<String>,
    pub user_id: Option<String>,
    pub silenced_count: u32,
    pub labels: BTreeMap<String, String>,
    pub annotations: BTreeMap<String, String>,
}

#[derive(Debug, Clone)]
pub struct AlertsOverviewService {
    http: reqwest::Client,
    alertmanager: String,
}

impl AlertsOverviewService {
    pub fn new(http: reqwest::Client, alertmanager: String) -> Self {
        Self { http, alertmanager }
    }

    pub async fn overview(&self, timeout: Duration) -> Result<AlertsOverview> {
        let base: Url = self.alertmanager.parse()?;
        let url = base.join("/api/v2/alerts")?;
        let resp = self.http.get(url).timeout(timeout).send().await?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            return Err(anyhow!("alertmanager responded {}: {}", status, body));
        }

        let upstream = serde_json::from_str::<Vec<UpstreamAlert>>(&body)?;
        let mut alerts = upstream.into_iter().map(AlertRecord::from_upstream).collect::<Vec<_>>();
        alerts.sort_by(|a, b| {
            severity_rank(&b.severity)
                .cmp(&severity_rank(&a.severity))
                .then_with(|| b.starts_at.cmp(&a.starts_at))
                .then_with(|| a.name.cmp(&b.name))
        });

        let mut severity_counts: HashMap<String, u32> = HashMap::new();
        let mut source_counts: HashMap<String, u32> = HashMap::new();
        let mut unique_sources = HashSet::new();
        let mut active = 0_u32;
        let mut critical = 0_u32;
        let mut silenced = 0_u32;

        for alert in &alerts {
            *severity_counts.entry(alert.severity.clone()).or_default() += 1;
            *source_counts.entry(alert.source.clone()).or_default() += 1;
            if !alert.source.is_empty() {
                unique_sources.insert(alert.source.clone());
            }
            if alert.severity == "critical" {
                critical += 1;
            }
            if is_silenced(alert) {
                silenced += 1;
            } else {
                active += 1;
            }
        }

        Ok(AlertsOverview {
            totals: AlertTotals {
                total: alerts.len() as u32,
                active,
                critical,
                silenced,
                unique_sources: unique_sources.len() as u32,
            },
            severity_breakdown: sort_breakdown(severity_counts),
            source_breakdown: sort_breakdown(source_counts).into_iter().take(8).collect(),
            alerts,
        })
    }
}

impl AlertRecord {
    fn from_upstream(alert: UpstreamAlert) -> Self {
        let severity = alert
            .labels
            .get("severity")
            .map(|v| v.trim().to_lowercase())
            .filter(|v| !v.is_empty())
            .unwrap_or_else(|| "unknown".to_string());
        let state = alert
            .status
            .state
            .as_deref()
            .map(str::trim)
            .filter(|v| !v.is_empty())
            .unwrap_or("firing")
            .to_lowercase();
        let source = first_non_empty(&[
            alert.labels.get("instance").map(String::as_str),
            alert.labels.get("job").map(String::as_str),
            alert.labels.get("source_ip").map(String::as_str),
            alert.labels.get("user_id").map(String::as_str),
        ])
        .unwrap_or("unknown")
        .to_string();
        let name = first_non_empty(&[
            alert.labels.get("alertname").map(String::as_str),
            alert.labels.get("rule_id").map(String::as_str),
        ])
        .unwrap_or("Alert")
        .to_string();
        let description = first_non_empty(&[
            alert.annotations.get("description").map(String::as_str),
            alert.annotations.get("summary").map(String::as_str),
        ])
        .unwrap_or("—")
        .to_string();
        let summary = alert
            .annotations
            .get("summary")
            .cloned()
            .filter(|value| !value.trim().is_empty())
            .unwrap_or_else(|| description.clone());

        Self {
            fingerprint: alert.fingerprint,
            name,
            severity,
            state,
            source,
            summary,
            description,
            starts_at: alert.starts_at,
            ends_at: alert.ends_at,
            rule_id: alert.labels.get("rule_id").cloned().filter(|v| !v.trim().is_empty()),
            source_ip: alert.labels.get("source_ip").cloned().filter(|v| !v.trim().is_empty()),
            user_id: alert.labels.get("user_id").cloned().filter(|v| !v.trim().is_empty()),
            silenced_count: alert.status.silenced_by.len() as u32,
            labels: alert.labels.into_iter().collect(),
            annotations: alert.annotations.into_iter().collect(),
        }
    }
}

#[derive(Debug, Deserialize)]
struct UpstreamAlert {
    fingerprint: String,
    #[serde(default)]
    status: UpstreamStatus,
    #[serde(default)]
    labels: HashMap<String, String>,
    #[serde(default)]
    annotations: HashMap<String, String>,
    #[serde(default, rename = "startsAt")]
    starts_at: Option<String>,
    #[serde(default, rename = "endsAt")]
    ends_at: Option<String>,
}

#[derive(Debug, Default, Deserialize)]
struct UpstreamStatus {
    state: Option<String>,
    #[serde(default, rename = "silencedBy")]
    silenced_by: Vec<String>,
}

fn sort_breakdown(input: HashMap<String, u32>) -> Vec<AlertBreakdown> {
    let mut rows = input
        .into_iter()
        .map(|(name, count)| AlertBreakdown { name, count })
        .collect::<Vec<_>>();
    rows.sort_by(|a, b| {
        b.count
            .cmp(&a.count)
            .then_with(|| severity_rank(&b.name).cmp(&severity_rank(&a.name)))
            .then_with(|| a.name.cmp(&b.name))
    });
    rows
}

fn is_silenced(alert: &AlertRecord) -> bool {
    alert.silenced_count > 0 || alert.state == "suppressed"
}

fn first_non_empty<'a>(values: &[Option<&'a str>]) -> Option<&'a str> {
    values
        .iter()
        .flatten()
        .map(|value| value.trim())
        .find(|value| !value.is_empty())
}

