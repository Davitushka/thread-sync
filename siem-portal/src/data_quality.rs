use std::collections::BTreeMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use reqwest::Url;
use serde::Serialize;
use serde_json::Value;

use crate::config::ClickHouseConfig;

const DEFAULT_WINDOW_HOURS: u16 = 24;
const MIN_WINDOW_HOURS: u16 = 1;
const MAX_WINDOW_HOURS: u16 = 168;
const PARSER_OK_QUERY: &str = "sum(rate(siem_parser_events_parsed_total{status=\"ok\"}[5m])) or vector(0)";
const PARSER_ERROR_QUERY: &str = "sum(rate(siem_parser_events_parsed_total{status=\"error\"}[5m])) or vector(0)";
const CONSUMER_LAG_QUERY: &str = "siem:kafka_consumer_lag:sum or vector(0)";

#[derive(Debug, Clone, Copy)]
pub struct DataQualityRequest {
    pub window_hours: u16,
    pub step_sec: u32,
    pub lag_window_hours: u16,
}

impl DataQualityRequest {
    pub fn from_query(hours: Option<u16>) -> Self {
        let window_hours = hours
            .unwrap_or(DEFAULT_WINDOW_HOURS)
            .clamp(MIN_WINDOW_HOURS, MAX_WINDOW_HOURS);
        let step_sec = match window_hours {
            0..=6 => 120,
            7..=24 => 300,
            25..=72 => 900,
            _ => 1800,
        };
        let lag_window_hours = window_hours.max(24).min(48);
        Self {
            window_hours,
            step_sec,
            lag_window_hours,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct DataQualityDashboard {
    pub window_hours: u16,
    pub step_sec: u32,
    pub lag_window_hours: u16,
    pub kpis: DataQualityKpis,
    pub lag_series: Vec<DataQualityLagPoint>,
    pub parser_series: Vec<DataQualityParserPoint>,
    pub consumer_lag_series: Vec<DataQualityConsumerLagPoint>,
}

#[derive(Debug, Clone, Serialize, Default)]
pub struct DataQualityKpis {
    pub total_events: u64,
    pub missing_source_ip_pct: f64,
    pub p95_ingest_lag_ms: f64,
    pub unique_source_types: u64,
    pub parser_ok_rate: f64,
    pub parser_error_rate: f64,
    pub consumer_lag: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DataQualityLagPoint {
    pub bucket: String,
    pub p95_lag_ms: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DataQualityParserPoint {
    pub ts: i64,
    pub ok_rate: f64,
    pub error_rate: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct DataQualityConsumerLagPoint {
    pub ts: i64,
    pub lag: f64,
}

#[derive(Debug, Clone)]
pub struct DataQualityService {
    http: reqwest::Client,
    clickhouse: ClickHouseConfig,
    prometheus: String,
}

#[derive(Debug, Clone)]
struct MetricPoint {
    ts: i64,
    value: f64,
}

impl DataQualityService {
    pub fn new(http: reqwest::Client, clickhouse: ClickHouseConfig, prometheus: String) -> Self {
        Self {
            http,
            clickhouse,
            prometheus,
        }
    }

    pub async fn dashboard(&self, request: DataQualityRequest, timeout: Duration) -> Result<DataQualityDashboard> {
        let db = ident(&self.clickhouse.database)?;
        let window_hours = request.window_hours;
        let lag_window_hours = request.lag_window_hours;
        let end = unix_now();
        let start = end.saturating_sub((window_hours as i64) * 3600);

        let sql_kpis = format!(
            "SELECT \
                count() AS total_events, \
                round(100 * countIf(source_ip IS NULL) / nullIf(count(), 0), 2) AS missing_source_ip_pct, \
                round(quantileTDigest(0.95)(ingest_lag_ms), 1) AS p95_ingest_lag_ms, \
                uniqExact(source_type) AS unique_source_types \
            FROM {db}.events \
            WHERE timestamp >= now() - INTERVAL {window_hours} HOUR \
            FORMAT JSONEachRow"
        );
        let sql_lag = format!(
            "SELECT \
                formatDateTime(toStartOfHour(timestamp), '%Y-%m-%dT%H:%i:%S.000Z') AS bucket_iso, \
                quantileTDigest(0.95)(ingest_lag_ms) AS p95_lag_ms \
            FROM {db}.events \
            WHERE timestamp >= now() - INTERVAL {lag_window_hours} HOUR \
            GROUP BY toStartOfHour(timestamp) \
            ORDER BY toStartOfHour(timestamp) \
            FORMAT JSONEachRow"
        );

        let (kpis_body, lag_body, parser_ok_rate, parser_error_rate, consumer_lag, parser_ok_series, parser_error_series, consumer_lag_series) =
            tokio::try_join!(
                self.query_clickhouse(&sql_kpis, timeout),
                self.query_clickhouse(&sql_lag, timeout),
                self.instant_scalar(PARSER_OK_QUERY, timeout),
                self.instant_scalar(PARSER_ERROR_QUERY, timeout),
                self.instant_scalar(CONSUMER_LAG_QUERY, timeout),
                self.range_series_sum(PARSER_OK_QUERY, start, end, request.step_sec, timeout),
                self.range_series_sum(PARSER_ERROR_QUERY, start, end, request.step_sec, timeout),
                self.range_series_sum(CONSUMER_LAG_QUERY, start, end, request.step_sec, timeout),
            )?;

        let mut kpis = parse_rows(kpis_body)?
            .into_iter()
            .next()
            .map(DataQualityKpis::from_json)
            .transpose()?
            .unwrap_or_default();
        kpis.parser_ok_rate = parser_ok_rate;
        kpis.parser_error_rate = parser_error_rate;
        kpis.consumer_lag = consumer_lag;

        Ok(DataQualityDashboard {
            window_hours,
            step_sec: request.step_sec,
            lag_window_hours,
            kpis,
            lag_series: parse_rows(lag_body)?
                .into_iter()
                .map(DataQualityLagPoint::from_json)
                .collect::<Result<Vec<_>>>()?,
            parser_series: merge_parser_series(parser_ok_series, parser_error_series),
            consumer_lag_series: consumer_lag_series
                .into_iter()
                .map(|point| DataQualityConsumerLagPoint {
                    ts: point.ts,
                    lag: point.value,
                })
                .collect(),
        })
    }

    async fn instant_scalar(&self, query: &str, timeout: Duration) -> Result<f64> {
        let payload = self.query_prometheus("/api/v1/query", query, &[], timeout).await?;
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

    async fn range_series_sum(
        &self,
        query: &str,
        start: i64,
        end: i64,
        step: u32,
        timeout: Duration,
    ) -> Result<Vec<MetricPoint>> {
        let payload = self
            .query_prometheus(
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

    async fn query_prometheus(
        &self,
        path: &str,
        query: &str,
        extra: &[(&str, String)],
        timeout: Duration,
    ) -> Result<Value> {
        let base: Url = self.prometheus.parse()?;
        let mut url = base.join(path)?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("query", query);
            for (key, value) in extra {
                pairs.append_pair(key, value);
            }
        }
        let resp = self.http.get(url).timeout(timeout).send().await?;
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

    async fn query_clickhouse(&self, sql: &str, timeout: Duration) -> Result<String> {
        let mut url = Url::parse(&self.clickhouse.url)?;
        {
            let mut pairs = url.query_pairs_mut();
            pairs.append_pair("database", &self.clickhouse.database);
            pairs.append_pair("query", sql);
        }
        let resp = self
            .http
            .get(url)
            .basic_auth(&self.clickhouse.user, Some(&self.clickhouse.password))
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

impl DataQualityKpis {
    fn from_json(v: Value) -> Result<Self> {
        Ok(Self {
            total_events: as_u64(&v, "total_events"),
            missing_source_ip_pct: as_f64(&v, "missing_source_ip_pct"),
            p95_ingest_lag_ms: as_f64(&v, "p95_ingest_lag_ms"),
            unique_source_types: as_u64(&v, "unique_source_types"),
            ..Self::default()
        })
    }
}

impl DataQualityLagPoint {
    fn from_json(v: Value) -> Result<Self> {
        Ok(Self {
            bucket: get_str(&v, "bucket_iso"),
            p95_lag_ms: as_f64(&v, "p95_lag_ms"),
        })
    }
}

fn merge_parser_series(ok: Vec<MetricPoint>, error: Vec<MetricPoint>) -> Vec<DataQualityParserPoint> {
    let mut merged: BTreeMap<i64, DataQualityParserPoint> = BTreeMap::new();
    for point in ok {
        merged.entry(point.ts).or_insert_with(|| DataQualityParserPoint {
            ts: point.ts,
            ok_rate: 0.0,
            error_rate: 0.0,
        });
        if let Some(row) = merged.get_mut(&point.ts) {
            row.ok_rate = point.value;
        }
    }
    for point in error {
        merged.entry(point.ts).or_insert_with(|| DataQualityParserPoint {
            ts: point.ts,
            ok_rate: 0.0,
            error_rate: 0.0,
        });
        if let Some(row) = merged.get_mut(&point.ts) {
            row.error_rate = point.value;
        }
    }
    merged.into_values().collect()
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

fn unix_now() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_secs() as i64
}

fn parse_prom_value_pair(value: &Value) -> Option<f64> {
    let arr = value.as_array()?;
    let raw = arr.get(1)?.as_str()?;
    raw.parse::<f64>().ok()
}

fn parse_prom_series_pair(value: &Value) -> Option<(i64, f64)> {
    let arr = value.as_array()?;
    let ts = arr.first()?.as_f64()? as i64;
    let raw = arr.get(1)?.as_str()?;
    Some((ts, raw.parse::<f64>().ok()?))
}
