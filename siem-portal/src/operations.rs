use std::collections::BTreeMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use reqwest::Url;
use serde::Serialize;
use serde_json::Value;

const DEFAULT_WINDOW_HOURS: u16 = 24;
const MIN_WINDOW_HOURS: u16 = 1;
const MAX_WINDOW_HOURS: u16 = 168;

const COMPONENT_STATUS_QUERY: &str =
    "max by(job) (up{job=~\"vector-aggregator|siem-parser|redpanda|clickhouse|correlator|alertmanager|prometheus|node-exporter|cadvisor|grafana|loki|minio|redis\"})";
const CLICKHOUSE_SELECT_QUERY: &str = "sum(rate(ClickHouseProfileEvents_SelectQuery[2m])) or vector(0)";
const CLICKHOUSE_INSERT_QUERY: &str = "sum(rate(ClickHouseProfileEvents_InsertQuery[2m])) or vector(0)";
const CLICKHOUSE_FAILED_QUERY: &str = "sum(rate(ClickHouseProfileEvents_FailedQuery[2m])) or vector(0)";
const REDPANDA_RECORDS_QUERY: &str =
    "sum(rate(redpanda_kafka_records_produced_total{redpanda_namespace=\"kafka\",redpanda_topic=\"siem.events\"}[2m])) or vector(0)";
const VECTOR_HTTP_INGEST_QUERY: &str =
    "sum(rate(vector_component_received_events_total{component_id=\"http_ingest\"}[2m])) or vector(0)";
const VECTOR_TO_REDPANDA_QUERY: &str =
    "sum(rate(vector_component_received_events_total{component_id=\"to_redpanda\"}[2m])) or vector(0)";
const DETECTION_PROCESSED_QUERY: &str =
    "sum(rate(detection_events_processed_total{job=\"correlator\"}[2m])) or vector(0)";
const PARSER_IN_FLIGHT_QUERY: &str = "sum(siem_parser_events_in_flight) or vector(0)";
const FIRING_ALERTS_QUERY: &str = "count(ALERTS{alertstate=\"firing\"}) or vector(0)";
const PARSE_ERRORS_24H_QUERY: &str = "sum(increase(detection_parse_errors_total[24h])) or vector(0)";
const DROPPED_ALERTS_24H_QUERY: &str = "sum(increase(detection_alerts_dropped_total[24h])) or vector(0)";

#[derive(Debug, Clone, Copy)]
pub struct OperationsRequest {
    pub window_hours: u16,
    pub step_sec: u32,
}

impl OperationsRequest {
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
        Self {
            window_hours,
            step_sec,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct OperationsDashboard {
    pub window_hours: u16,
    pub step_sec: u32,
    pub totals: OperationsTotals,
    pub component_status: Vec<OperationsComponentStatus>,
    pub clickhouse_series: Vec<OperationsClickHousePoint>,
    pub vector_series: Vec<OperationsVectorPoint>,
    pub pipeline_series: Vec<OperationsPipelinePoint>,
}

#[derive(Debug, Clone, Serialize)]
pub struct OperationsTotals {
    pub clickhouse_select_qps: f64,
    pub clickhouse_insert_qps: f64,
    pub redpanda_records_rate: f64,
    pub vector_ingest_rate: f64,
    pub vector_forward_rate: f64,
    pub detection_processed_rate: f64,
    pub firing_alerts: u64,
    pub parser_in_flight: u64,
    pub parse_errors_24h: u64,
    pub dropped_alerts_24h: u64,
    pub healthy_components: u32,
    pub total_components: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct OperationsComponentStatus {
    pub job: String,
    pub up: bool,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct OperationsClickHousePoint {
    pub ts: i64,
    pub select_qps: f64,
    pub insert_qps: f64,
    pub failed_qps: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct OperationsVectorPoint {
    pub ts: i64,
    pub http_ingest_eps: f64,
    pub to_redpanda_eps: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct OperationsPipelinePoint {
    pub ts: i64,
    pub redpanda_records_eps: f64,
    pub detection_processed_eps: f64,
}

#[derive(Debug, Clone)]
pub struct OperationsService {
    http: reqwest::Client,
    prometheus: String,
}

#[derive(Debug, Clone)]
struct MetricPoint {
    ts: i64,
    value: f64,
}

impl OperationsService {
    pub fn new(http: reqwest::Client, prometheus: String) -> Self {
        Self { http, prometheus }
    }

    pub async fn dashboard(&self, request: OperationsRequest, timeout: Duration) -> Result<OperationsDashboard> {
        let end = unix_now();
        let start = end.saturating_sub((request.window_hours as i64) * 3600);

        let (
            clickhouse_select_qps,
            clickhouse_insert_qps,
            redpanda_records_rate,
            vector_ingest_rate,
            vector_forward_rate,
            detection_processed_rate,
            firing_alerts,
            parser_in_flight,
            parse_errors_24h,
            dropped_alerts_24h,
            component_status,
            clickhouse_select_series,
            clickhouse_insert_series,
            clickhouse_failed_series,
            vector_ingest_series,
            vector_forward_series,
            redpanda_records_series,
            detection_processed_series,
        ) = tokio::try_join!(
            self.instant_scalar(CLICKHOUSE_SELECT_QUERY, timeout),
            self.instant_scalar(CLICKHOUSE_INSERT_QUERY, timeout),
            self.instant_scalar(REDPANDA_RECORDS_QUERY, timeout),
            self.instant_scalar(VECTOR_HTTP_INGEST_QUERY, timeout),
            self.instant_scalar(VECTOR_TO_REDPANDA_QUERY, timeout),
            self.instant_scalar(DETECTION_PROCESSED_QUERY, timeout),
            self.instant_scalar(FIRING_ALERTS_QUERY, timeout),
            self.instant_scalar(PARSER_IN_FLIGHT_QUERY, timeout),
            self.instant_scalar(PARSE_ERRORS_24H_QUERY, timeout),
            self.instant_scalar(DROPPED_ALERTS_24H_QUERY, timeout),
            self.component_status(timeout),
            self.range_series_sum(CLICKHOUSE_SELECT_QUERY, start, end, request.step_sec, timeout),
            self.range_series_sum(CLICKHOUSE_INSERT_QUERY, start, end, request.step_sec, timeout),
            self.range_series_sum(CLICKHOUSE_FAILED_QUERY, start, end, request.step_sec, timeout),
            self.range_series_sum(VECTOR_HTTP_INGEST_QUERY, start, end, request.step_sec, timeout),
            self.range_series_sum(VECTOR_TO_REDPANDA_QUERY, start, end, request.step_sec, timeout),
            self.range_series_sum(REDPANDA_RECORDS_QUERY, start, end, request.step_sec, timeout),
            self.range_series_sum(DETECTION_PROCESSED_QUERY, start, end, request.step_sec, timeout),
        )?;

        let healthy_components = component_status.iter().filter(|item| item.up).count() as u32;
        let total_components = component_status.len() as u32;

        Ok(OperationsDashboard {
            window_hours: request.window_hours,
            step_sec: request.step_sec,
            totals: OperationsTotals {
                clickhouse_select_qps,
                clickhouse_insert_qps,
                redpanda_records_rate,
                vector_ingest_rate,
                vector_forward_rate,
                detection_processed_rate,
                firing_alerts: firing_alerts.max(0.0) as u64,
                parser_in_flight: parser_in_flight.max(0.0) as u64,
                parse_errors_24h: parse_errors_24h.max(0.0) as u64,
                dropped_alerts_24h: dropped_alerts_24h.max(0.0) as u64,
                healthy_components,
                total_components,
            },
            component_status,
            clickhouse_series: merge_clickhouse_series(
                clickhouse_select_series,
                clickhouse_insert_series,
                clickhouse_failed_series,
            ),
            vector_series: merge_vector_series(vector_ingest_series, vector_forward_series),
            pipeline_series: merge_pipeline_series(redpanda_records_series, detection_processed_series),
        })
    }

    async fn instant_scalar(&self, query: &str, timeout: Duration) -> Result<f64> {
        let payload = self.query("/api/v1/query", query, &[], timeout).await?;
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

    async fn component_status(&self, timeout: Duration) -> Result<Vec<OperationsComponentStatus>> {
        let payload = self
            .query("/api/v1/query", COMPONENT_STATUS_QUERY, &[], timeout)
            .await?;
        let result = payload
            .get("data")
            .and_then(|v| v.get("result"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let mut rows = result
            .iter()
            .filter_map(|row| {
                let job = row
                    .get("metric")
                    .and_then(|v| v.get("job"))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .unwrap_or_default()
                    .to_string();
                let value = row.get("value").and_then(parse_prom_value_pair)?;
                if job.is_empty() {
                    return None;
                }
                Some(OperationsComponentStatus {
                    job,
                    up: value >= 1.0,
                    value,
                })
            })
            .collect::<Vec<_>>();

        rows.sort_by(|a, b| a.job.cmp(&b.job));
        Ok(rows)
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
            .query(
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

    async fn query(
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
}

fn merge_clickhouse_series(
    select: Vec<MetricPoint>,
    insert: Vec<MetricPoint>,
    failed: Vec<MetricPoint>,
) -> Vec<OperationsClickHousePoint> {
    let mut merged: BTreeMap<i64, OperationsClickHousePoint> = BTreeMap::new();
    for point in select {
        merged.entry(point.ts).or_insert_with(|| OperationsClickHousePoint {
            ts: point.ts,
            select_qps: 0.0,
            insert_qps: 0.0,
            failed_qps: 0.0,
        });
        if let Some(row) = merged.get_mut(&point.ts) {
            row.select_qps = point.value;
        }
    }
    for point in insert {
        merged.entry(point.ts).or_insert_with(|| OperationsClickHousePoint {
            ts: point.ts,
            select_qps: 0.0,
            insert_qps: 0.0,
            failed_qps: 0.0,
        });
        if let Some(row) = merged.get_mut(&point.ts) {
            row.insert_qps = point.value;
        }
    }
    for point in failed {
        merged.entry(point.ts).or_insert_with(|| OperationsClickHousePoint {
            ts: point.ts,
            select_qps: 0.0,
            insert_qps: 0.0,
            failed_qps: 0.0,
        });
        if let Some(row) = merged.get_mut(&point.ts) {
            row.failed_qps = point.value;
        }
    }
    merged.into_values().collect()
}

fn merge_vector_series(ingest: Vec<MetricPoint>, forward: Vec<MetricPoint>) -> Vec<OperationsVectorPoint> {
    let mut merged: BTreeMap<i64, OperationsVectorPoint> = BTreeMap::new();
    for point in ingest {
        merged.entry(point.ts).or_insert_with(|| OperationsVectorPoint {
            ts: point.ts,
            http_ingest_eps: 0.0,
            to_redpanda_eps: 0.0,
        });
        if let Some(row) = merged.get_mut(&point.ts) {
            row.http_ingest_eps = point.value;
        }
    }
    for point in forward {
        merged.entry(point.ts).or_insert_with(|| OperationsVectorPoint {
            ts: point.ts,
            http_ingest_eps: 0.0,
            to_redpanda_eps: 0.0,
        });
        if let Some(row) = merged.get_mut(&point.ts) {
            row.to_redpanda_eps = point.value;
        }
    }
    merged.into_values().collect()
}

fn merge_pipeline_series(redpanda: Vec<MetricPoint>, detection: Vec<MetricPoint>) -> Vec<OperationsPipelinePoint> {
    let mut merged: BTreeMap<i64, OperationsPipelinePoint> = BTreeMap::new();
    for point in redpanda {
        merged.entry(point.ts).or_insert_with(|| OperationsPipelinePoint {
            ts: point.ts,
            redpanda_records_eps: 0.0,
            detection_processed_eps: 0.0,
        });
        if let Some(row) = merged.get_mut(&point.ts) {
            row.redpanda_records_eps = point.value;
        }
    }
    for point in detection {
        merged.entry(point.ts).or_insert_with(|| OperationsPipelinePoint {
            ts: point.ts,
            redpanda_records_eps: 0.0,
            detection_processed_eps: 0.0,
        });
        if let Some(row) = merged.get_mut(&point.ts) {
            row.detection_processed_eps = point.value;
        }
    }
    merged.into_values().collect()
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
