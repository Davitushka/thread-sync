//! Shared helpers for ClickHouse and Prometheus queries, JSON parsing,
//! and common types used across multiple dashboard modules.

use std::collections::BTreeMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use reqwest::Url;
use serde::Serialize;
use serde_json::Value;

use crate::config::ClickHouseConfig;

// ── Common types ──────────────────────────────────────────────────────

#[derive(Debug, Clone, Serialize)]
pub struct MetricPoint {
    pub ts: i64,
    pub value: f64,
}

// ── Severity ranking (shared by alerts and detections) ────────────────

pub fn severity_rank(value: &str) -> u8 {
    match value {
        "critical" => 7,
        "high" | "error" => 6,
        "warning" | "medium" => 5,
        "info" | "low" => 4,
        "debug" => 3,
        _ => 1,
    }
}

// ── ClickHouse helpers ────────────────────────────────────────────────

pub async fn query_clickhouse(
    http: &reqwest::Client,
    cfg: &ClickHouseConfig,
    sql: &str,
    timeout: Duration,
) -> Result<String> {
    let mut url = Url::parse(&cfg.url)?;
    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("database", &cfg.database);
        pairs.append_pair("query", sql);
        pairs.append_pair("user", &cfg.user);
        pairs.append_pair("password", &cfg.password);
    }
    let resp = http.get(url).timeout(timeout).send().await?;
    let status = resp.status();
    let body = resp.text().await?;
    if !status.is_success() {
        return Err(anyhow!("clickhouse responded {}: {}", status, body));
    }
    Ok(body)
}

pub fn parse_rows(body: String) -> Result<Vec<Value>> {
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

pub fn get_str(v: &Value, key: &str) -> String {
    v.get(key)
        .and_then(Value::as_str)
        .unwrap_or_default()
        .to_string()
}

pub fn get_opt_str(v: &Value, key: &str) -> Option<String> {
    v.get(key)
        .and_then(Value::as_str)
        .map(str::trim)
        .filter(|s| !s.is_empty())
        .map(ToString::to_string)
}

/// Parse a JSON value as u64, falling back through i64 and string representations.
pub fn as_u64(v: &Value, key: &str) -> u64 {
    v.get(key)
        .and_then(Value::as_u64)
        .or_else(|| v.get(key).and_then(Value::as_i64).map(|n| n.max(0) as u64))
        .or_else(|| v.get(key).and_then(Value::as_str).and_then(|s| s.parse::<u64>().ok()))
        .unwrap_or(0)
}

pub fn as_f64(v: &Value, key: &str) -> f64 {
    v.get(key)
        .and_then(Value::as_f64)
        .or_else(|| v.get(key).and_then(Value::as_str).and_then(|s| s.parse::<f64>().ok()))
        .unwrap_or(0.0)
}

pub fn ident(value: &str) -> Result<&str> {
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

// ── Prometheus helpers ────────────────────────────────────────────────

pub async fn query_prometheus(
    http: &reqwest::Client,
    prometheus_url: &str,
    path: &str,
    query: &str,
    extra: &[(&str, String)],
    timeout: Duration,
) -> Result<Value> {
    let base: Url = prometheus_url.parse()?;
    let mut url = base.join(path)?;
    {
        let mut pairs = url.query_pairs_mut();
        pairs.append_pair("query", query);
        for (key, value) in extra {
            pairs.append_pair(key, value);
        }
    }
    let resp = http.get(url).timeout(timeout).send().await?;
    let status = resp.status();
    let body = resp.text().await?;
    if !status.is_success() {
        return Err(anyhow!("prometheus responded {}: {}", status, body));
    }
    let payload = serde_json::from_str::<Value>(&body)?;
    if payload.get("status").and_then(Value::as_str) != Some("success") {
        return Err(anyhow!("prometheus query failed: {}", body));
    }
    Ok(payload)
}

pub fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

pub fn parse_prom_value_pair(value: &Value) -> Option<f64> {
    let arr = value.as_array()?;
    let raw = arr.get(1)?.as_str()?;
    raw.parse::<f64>().ok()
}

pub fn parse_prom_series_pair(value: &Value) -> Option<(i64, f64)> {
    let arr = value.as_array()?;
    let ts = arr.first()?.as_f64()? as i64;
    let raw = arr.get(1)?.as_str()?;
    Some((ts, raw.parse::<f64>().ok()?))
}

pub async fn range_series_sum(
    http: &reqwest::Client,
    prometheus_url: &str,
    query: &str,
    start: i64,
    end: i64,
    step: u32,
    timeout: Duration,
) -> Result<Vec<MetricPoint>> {
    let payload = query_prometheus(
        http,
        prometheus_url,
        "/api/v1/query_range",
        query,
        &[
            ("start", start.to_string()),
            ("end", end.to_string()),
            ("step", step.to_string()),
        ],
        timeout,
    )
    .await?;

    let result = payload
        .get("data")
        .and_then(|v| v.get("result"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();

    let mut merged: BTreeMap<i64, f64> = BTreeMap::new();
    for series in result {
        let values = series
            .get("values")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();
        for pair in values {
            if let Some((ts, value)) = parse_prom_series_pair(&pair) {
                *merged.entry(ts).or_insert(0.0) += value;
            }
        }
    }

    Ok(merged
        .into_iter()
        .map(|(ts, value)| MetricPoint { ts, value })
        .collect())
}

pub async fn instant_scalar_sum(
    http: &reqwest::Client,
    prometheus_url: &str,
    query: &str,
    timeout: Duration,
) -> Result<f64> {
    let payload = query_prometheus(http, prometheus_url, "/api/v1/query", query, &[], timeout).await?;
    let result = payload
        .get("data")
        .and_then(|v| v.get("result"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    Ok(result
        .iter()
        .filter_map(|row| row.get("value").and_then(parse_prom_value_pair))
        .sum())
}

pub async fn instant_scalar_max(
    http: &reqwest::Client,
    prometheus_url: &str,
    query: &str,
    timeout: Duration,
) -> Result<f64> {
    let payload = query_prometheus(http, prometheus_url, "/api/v1/query", query, &[], timeout).await?;
    let result = payload
        .get("data")
        .and_then(|v| v.get("result"))
        .and_then(Value::as_array)
        .cloned()
        .unwrap_or_default();
    if result.is_empty() {
        return Ok(0.0);
    }
    Ok(result
        .iter()
        .filter_map(|row| row.get("value").and_then(parse_prom_value_pair))
        .fold(0.0_f64, f64::max))
}
