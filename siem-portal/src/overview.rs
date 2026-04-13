use anyhow::{anyhow, Result};
use reqwest::Url;
use serde::Serialize;
use serde_json::Value;

use crate::config::ClickHouseConfig;

const OVERVIEW_WINDOW_HOURS: u8 = 24;

#[derive(Debug, Clone, Serialize)]
pub struct OverviewDashboard {
    pub window_hours: u8,
    pub kpis: OverviewKpis,
    pub events_per_minute: Vec<OverviewMinutePoint>,
    pub severity_breakdown: Vec<OverviewSeverityBucket>,
    pub source_breakdown: Vec<OverviewSourceBucket>,
    pub top_source_ips: Vec<OverviewTopSourceIp>,
    pub recent_security_events: Vec<OverviewRecentEvent>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OverviewKpis {
    pub total_events_24h: u64,
    pub critical_events_24h: u64,
    pub error_pct_24h: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct OverviewMinutePoint {
    pub minute: String,
    pub events: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct OverviewSeverityBucket {
    pub severity: String,
    pub events: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct OverviewSourceBucket {
    pub source_type: String,
    pub events: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct OverviewTopSourceIp {
    pub source_ip: String,
    pub events: u64,
    pub threats: u64,
}

#[derive(Debug, Clone, Serialize)]
pub struct OverviewRecentEvent {
    pub timestamp: String,
    pub event_id: String,
    pub source_type: String,
    pub severity: String,
    pub host: String,
    pub source_ip: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone)]
pub struct OverviewService {
    http: reqwest::Client,
    cfg: ClickHouseConfig,
}

impl OverviewService {
    pub fn new(http: reqwest::Client, cfg: ClickHouseConfig) -> Self {
        Self { http, cfg }
    }

    pub async fn dashboard(&self, timeout: std::time::Duration) -> Result<OverviewDashboard> {
        let db = ident(&self.cfg.database)?;
        let sql_kpis = format!(
            "SELECT \
                countMerge(event_count) AS total_events_24h, \
                countMergeIf(event_count, severity = 'critical') AS critical_events_24h, \
                round(toFloat64(countMergeIf(event_count, toUInt8(severity) >= 3)) / nullIf(countMerge(event_count), 0) * 100, 2) AS error_pct_24h \
            FROM {db}.events_per_minute_agg \
            WHERE minute >= now() - INTERVAL {OVERVIEW_WINDOW_HOURS} HOUR \
            FORMAT JSONEachRow"
        );
        let sql_events_per_minute = format!(
            "SELECT \
                formatDateTime(minute, '%Y-%m-%dT%H:%i:%S.000Z') AS minute_iso, \
                countMerge(event_count) AS events \
            FROM {db}.events_per_minute_agg \
            WHERE minute >= now() - INTERVAL {OVERVIEW_WINDOW_HOURS} HOUR \
            GROUP BY minute \
            ORDER BY minute \
            FORMAT JSONEachRow"
        );
        let sql_severity = format!(
            "SELECT \
                toString(severity) AS severity_text, \
                countMerge(event_count) AS events \
            FROM {db}.events_per_minute_agg \
            WHERE minute >= now() - INTERVAL {OVERVIEW_WINDOW_HOURS} HOUR \
            GROUP BY severity \
            ORDER BY events DESC \
            FORMAT JSONEachRow"
        );
        let sql_sources = format!(
            "SELECT \
                source_type, \
                countMerge(event_count) AS events \
            FROM {db}.events_per_minute_agg \
            WHERE minute >= now() - INTERVAL {OVERVIEW_WINDOW_HOURS} HOUR \
            GROUP BY source_type \
            ORDER BY events DESC \
            LIMIT 8 \
            FORMAT JSONEachRow"
        );
        let sql_top_ips = format!(
            "SELECT \
                toString(source_ip) AS source_ip_text, \
                countMerge(event_count) AS events, \
                countMerge(error_count) AS threats \
            FROM {db}.top_ips_agg \
            WHERE hour >= now() - INTERVAL {OVERVIEW_WINDOW_HOURS} HOUR \
            GROUP BY source_ip \
            ORDER BY threats DESC, events DESC \
            LIMIT 10 \
            FORMAT JSONEachRow"
        );
        let sql_recent = format!(
            "SELECT \
                formatDateTime(timestamp, '%Y-%m-%dT%H:%i:%S.%fZ') AS event_ts, \
                toString(event_id) AS event_id, \
                source_type, \
                toString(severity) AS severity_text, \
                host, \
                ifNull(toString(source_ip), '') AS source_ip_text, \
                left(message, 96) AS message \
            FROM {db}.events \
            WHERE timestamp >= now() - INTERVAL {OVERVIEW_WINDOW_HOURS} HOUR \
              AND (severity IN ('error', 'critical', 'warning') OR source_type = 'redis') \
            ORDER BY timestamp DESC \
            LIMIT 20 \
            FORMAT JSONEachRow"
        );

        let (kpis_body, events_body, severity_body, sources_body, ips_body, recent_body) = tokio::try_join!(
            self.query_json(&sql_kpis, timeout),
            self.query_json(&sql_events_per_minute, timeout),
            self.query_json(&sql_severity, timeout),
            self.query_json(&sql_sources, timeout),
            self.query_json(&sql_top_ips, timeout),
            self.query_json(&sql_recent, timeout),
        )?;

        let kpis = parse_rows(kpis_body)?
            .into_iter()
            .next()
            .map(OverviewKpis::from_json)
            .transpose()?
            .unwrap_or_default();
        let events_per_minute = parse_rows(events_body)?
            .into_iter()
            .map(OverviewMinutePoint::from_json)
            .collect::<Result<Vec<_>>>()?;
        let severity_breakdown = parse_rows(severity_body)?
            .into_iter()
            .map(OverviewSeverityBucket::from_json)
            .collect::<Result<Vec<_>>>()?;
        let source_breakdown = parse_rows(sources_body)?
            .into_iter()
            .map(OverviewSourceBucket::from_json)
            .collect::<Result<Vec<_>>>()?;
        let top_source_ips = parse_rows(ips_body)?
            .into_iter()
            .map(OverviewTopSourceIp::from_json)
            .collect::<Result<Vec<_>>>()?;
        let recent_security_events = parse_rows(recent_body)?
            .into_iter()
            .map(OverviewRecentEvent::from_json)
            .collect::<Result<Vec<_>>>()?;

        Ok(OverviewDashboard {
            window_hours: OVERVIEW_WINDOW_HOURS,
            kpis,
            events_per_minute,
            severity_breakdown,
            source_breakdown,
            top_source_ips,
            recent_security_events,
        })
    }

    async fn query_json(&self, sql: &str, timeout: std::time::Duration) -> Result<String> {
        let mut url = Url::parse(&self.cfg.url)?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("database", &self.cfg.database);
            pairs.append_pair("query", sql);
        }
        let resp = self
            .http
            .get(url)
            .basic_auth(&self.cfg.user, Some(&self.cfg.password))
            .timeout(timeout)
            .send()
            .await?;
        let status = resp.status();
        let body = resp.text().await?;
        if !status.is_success() {
            return Err(anyhow!("clickhouse responded {}: {}", status, body));
        }
        Ok(body)
    }
}

impl Default for OverviewKpis {
    fn default() -> Self {
        Self {
            total_events_24h: 0,
            critical_events_24h: 0,
            error_pct_24h: 0.0,
        }
    }
}

impl OverviewKpis {
    fn from_json(v: Value) -> Result<Self> {
        Ok(Self {
            total_events_24h: as_u64(&v, "total_events_24h"),
            critical_events_24h: as_u64(&v, "critical_events_24h"),
            error_pct_24h: as_f64(&v, "error_pct_24h"),
        })
    }
}

impl OverviewMinutePoint {
    fn from_json(v: Value) -> Result<Self> {
        Ok(Self {
            minute: get_str(&v, "minute_iso"),
            events: as_u64(&v, "events"),
        })
    }
}

impl OverviewSeverityBucket {
    fn from_json(v: Value) -> Result<Self> {
        Ok(Self {
            severity: get_str(&v, "severity_text"),
            events: as_u64(&v, "events"),
        })
    }
}

impl OverviewSourceBucket {
    fn from_json(v: Value) -> Result<Self> {
        Ok(Self {
            source_type: get_str(&v, "source_type"),
            events: as_u64(&v, "events"),
        })
    }
}

impl OverviewTopSourceIp {
    fn from_json(v: Value) -> Result<Self> {
        Ok(Self {
            source_ip: get_str(&v, "source_ip_text"),
            events: as_u64(&v, "events"),
            threats: as_u64(&v, "threats"),
        })
    }
}

impl OverviewRecentEvent {
    fn from_json(v: Value) -> Result<Self> {
        Ok(Self {
            timestamp: get_str(&v, "event_ts"),
            event_id: get_str(&v, "event_id"),
            source_type: get_str(&v, "source_type"),
            severity: get_str(&v, "severity_text"),
            host: get_str(&v, "host"),
            source_ip: get_opt_str(&v, "source_ip_text"),
            message: get_str(&v, "message"),
        })
    }
}

fn ident(value: &str) -> Result<&str> {
    if value
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || c == '_')
        && !value.is_empty()
    {
        Ok(value)
    } else {
        Err(anyhow!("invalid identifier"))
    }
}

fn parse_rows(body: String) -> Result<Vec<Value>> {
    let mut rows = Vec::new();
    for line in body.lines() {
        let line = line.trim();
        if line.is_empty() {
            continue;
        }
        rows.push(serde_json::from_str::<Value>(line)?);
    }
    Ok(rows)
}

fn get_str(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

fn get_opt_str(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
}

fn as_u64(v: &Value, key: &str) -> u64 {
    v.get(key)
        .and_then(Value::as_u64)
        .or_else(|| v.get(key).and_then(Value::as_i64).map(|n| n.max(0) as u64))
        .or_else(|| v.get(key).and_then(Value::as_str).and_then(|s| s.parse::<u64>().ok()))
        .unwrap_or(0)
}

fn as_f64(v: &Value, key: &str) -> f64 {
    v.get(key)
        .and_then(Value::as_f64)
        .or_else(|| v.get(key).and_then(Value::as_str).and_then(|s| s.parse::<f64>().ok()))
        .unwrap_or(0.0)
}
