use std::collections::BTreeMap;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

use anyhow::{anyhow, Result};
use reqwest::Url;
use serde::Serialize;
use serde_json::Value;

const DEFAULT_WINDOW_HOURS: u16 = 6;
const MIN_WINDOW_HOURS: u16 = 1;
const MAX_WINDOW_HOURS: u16 = 168;

#[derive(Debug, Clone, Copy)]
pub struct InfrastructureRequest {
    pub window_hours: u16,
    pub step_sec: u32,
}

impl InfrastructureRequest {
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

const HOST_CPU_QUERY: &str =
    "(100 - (avg(rate(node_cpu_seconds_total{mode=\"idle\",job=\"node-exporter\"}[2m])) * 100)) or (100 - (avg(rate(node_cpu_seconds_total{mode=\"idle\"}[2m])) * 100)) or vector(0)";
const HOST_MEMORY_QUERY: &str =
    "(1 - (node_memory_MemAvailable_bytes{job=\"node-exporter\"} / node_memory_MemTotal_bytes{job=\"node-exporter\"})) * 100 or (1 - (node_memory_MemAvailable_bytes / node_memory_MemTotal_bytes)) * 100 or vector(0)";
const HOST_DISK_QUERY: &str =
    "(max((1 - (node_filesystem_avail_bytes{job=\"node-exporter\",mountpoint=\"/\",fstype!=\"tmpfs\"} / node_filesystem_size_bytes{job=\"node-exporter\",mountpoint=\"/\",fstype!=\"tmpfs\"})) * 100)) or (max((1 - (node_filesystem_avail_bytes{mountpoint=\"/\",fstype!=\"tmpfs\"} / node_filesystem_size_bytes{mountpoint=\"/\",fstype!=\"tmpfs\"})) * 100)) or (max((1 - (node_filesystem_avail_bytes{job=\"node-exporter\",fstype!=\"tmpfs\",mountpoint=~\"/var/lib|/mnt/docker-desktop-disk|/mnt/host/[a-z]\"} / node_filesystem_size_bytes{job=\"node-exporter\",fstype!=\"tmpfs\",mountpoint=~\"/var/lib|/mnt/docker-desktop-disk|/mnt/host/[a-z]\"})) * 100)) or (max((1 - (node_filesystem_avail_bytes{fstype!=\"tmpfs\",mountpoint=~\"/var/lib|/mnt/docker-desktop-disk|/mnt/host/[a-z]\"} / node_filesystem_size_bytes{fstype!=\"tmpfs\",mountpoint=~\"/var/lib|/mnt/docker-desktop-disk|/mnt/host/[a-z]\"})) * 100)) or vector(0)";
const HOST_RX_QUERY: &str =
    "sum(rate(node_network_receive_bytes_total{job=\"node-exporter\",device!=\"lo\"}[2m])) or sum(rate(node_network_receive_bytes_total{device!=\"lo\"}[2m])) or vector(0)";
const HOST_TX_QUERY: &str =
    "sum(rate(node_network_transmit_bytes_total{job=\"node-exporter\",device!=\"lo\"}[2m])) or sum(rate(node_network_transmit_bytes_total{device!=\"lo\"}[2m])) or vector(0)";
const HOST_UPTIME_QUERY: &str =
    "(time() - node_boot_time_seconds{job=\"node-exporter\"}) or (time() - node_boot_time_seconds) or vector(0)";

const CONTAINER_COUNT_QUERY: &str =
    "count(container_memory_usage_bytes{job=\"cadvisor\",container!=\"\",container!=\"/\"}) or count(container_memory_usage_bytes{container!=\"\",container!=\"/\"}) or vector(0)";
const CONTAINER_TOTAL_CPU_QUERY: &str =
    "sum(rate(container_cpu_usage_seconds_total{job=\"cadvisor\",container!=\"\",container!=\"/\"}[2m])) * 100 or sum(rate(container_cpu_usage_seconds_total{container!=\"\",container!=\"/\"}[2m])) * 100 or vector(0)";
const CONTAINER_TOTAL_MEMORY_QUERY: &str =
    "sum(container_memory_usage_bytes{job=\"cadvisor\",container!=\"\",container!=\"/\"}) or sum(container_memory_usage_bytes{container!=\"\",container!=\"/\"}) or vector(0)";
const TOP_CPU_CONTAINERS_QUERY: &str =
    "topk(6, sum by(container) (rate(container_cpu_usage_seconds_total{job=\"cadvisor\",container!=\"\",container!=\"/\"}[5m])) * 100) or topk(6, sum by(container) (rate(container_cpu_usage_seconds_total{container!=\"\",container!=\"/\"}[5m])) * 100)";
const TOP_MEMORY_CONTAINERS_QUERY: &str =
    "topk(6, container_memory_usage_bytes{job=\"cadvisor\",container!=\"\",container!=\"/\"}) or topk(6, container_memory_usage_bytes{container!=\"\",container!=\"/\"})";
const COMPONENT_STATUS_QUERY: &str =
    "max by(job) (up{job=~\"vector-aggregator|siem-parser|redpanda|clickhouse|correlator|alertmanager|prometheus|node-exporter|cadvisor|grafana|loki|minio|redis\"})";

#[derive(Debug, Clone, Serialize)]
pub struct InfrastructureDashboard {
    pub window_hours: u16,
    pub step_sec: u32,
    pub host: InfrastructureHostSummary,
    pub cpu_series: Vec<MetricPoint>,
    pub network_rx_series: Vec<MetricPoint>,
    pub network_tx_series: Vec<MetricPoint>,
    pub top_cpu_containers: Vec<NamedMetric>,
    pub top_memory_containers: Vec<NamedMetric>,
    pub component_status: Vec<ComponentStatus>,
}

#[derive(Debug, Clone, Serialize)]
pub struct InfrastructureHostSummary {
    pub cpu_usage_pct: f64,
    pub memory_usage_pct: f64,
    pub disk_usage_pct: f64,
    pub network_rx_bps: f64,
    pub network_tx_bps: f64,
    pub uptime_sec: f64,
    pub container_count: u64,
    pub total_container_cpu_pct: f64,
    pub total_container_memory_bytes: f64,
    pub healthy_components: u32,
    pub total_components: u32,
}

#[derive(Debug, Clone, Serialize)]
pub struct MetricPoint {
    pub ts: i64,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct NamedMetric {
    pub name: String,
    pub value: f64,
}

#[derive(Debug, Clone, Serialize)]
pub struct ComponentStatus {
    pub job: String,
    pub up: bool,
    pub value: f64,
}

#[derive(Debug, Clone)]
pub struct InfrastructureService {
    http: reqwest::Client,
    prometheus: String,
}

impl InfrastructureService {
    pub fn new(http: reqwest::Client, prometheus: String) -> Self {
        Self { http, prometheus }
    }

    pub async fn dashboard(&self, request: InfrastructureRequest, timeout: Duration) -> Result<InfrastructureDashboard> {
        let end = unix_now();
        let start = end.saturating_sub((request.window_hours as i64) * 3600);

        let (
            cpu_now,
            memory_now,
            disk_now,
            rx_now,
            tx_now,
            uptime_now,
            container_count,
            total_container_cpu,
            total_container_memory,
            top_cpu,
            top_memory,
            component_status,
            cpu_series,
            network_rx_series,
            network_tx_series,
        ) = tokio::try_join!(
            self.instant_scalar(HOST_CPU_QUERY, timeout),
            self.instant_scalar(HOST_MEMORY_QUERY, timeout),
            self.instant_scalar(HOST_DISK_QUERY, timeout),
            self.instant_scalar(HOST_RX_QUERY, timeout),
            self.instant_scalar(HOST_TX_QUERY, timeout),
            self.instant_scalar(HOST_UPTIME_QUERY, timeout),
            self.instant_scalar(CONTAINER_COUNT_QUERY, timeout),
            self.instant_scalar(CONTAINER_TOTAL_CPU_QUERY, timeout),
            self.instant_scalar(CONTAINER_TOTAL_MEMORY_QUERY, timeout),
            self.instant_named_metrics(TOP_CPU_CONTAINERS_QUERY, "container", timeout),
            self.instant_named_metrics(TOP_MEMORY_CONTAINERS_QUERY, "container", timeout),
            self.component_status(timeout),
            self.range_series(HOST_CPU_QUERY, start, end, request.step_sec, timeout),
            self.range_series(HOST_RX_QUERY, start, end, request.step_sec, timeout),
            self.range_series(HOST_TX_QUERY, start, end, request.step_sec, timeout),
        )?;

        let healthy_components = component_status.iter().filter(|item| item.up).count() as u32;
        let total_components = component_status.len() as u32;

        Ok(InfrastructureDashboard {
            window_hours: request.window_hours,
            step_sec: request.step_sec,
            host: InfrastructureHostSummary {
                cpu_usage_pct: cpu_now,
                memory_usage_pct: memory_now,
                disk_usage_pct: disk_now,
                network_rx_bps: rx_now,
                network_tx_bps: tx_now,
                uptime_sec: uptime_now,
                container_count: container_count.max(0.0) as u64,
                total_container_cpu_pct: total_container_cpu,
                total_container_memory_bytes: total_container_memory,
                healthy_components,
                total_components,
            },
            cpu_series,
            network_rx_series,
            network_tx_series,
            top_cpu_containers: top_cpu,
            top_memory_containers: top_memory,
            component_status,
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

        if result.is_empty() {
            return Ok(0.0);
        }

        let max_value = result
            .iter()
            .filter_map(|row| row.get("value").and_then(parse_prom_value_pair))
            .fold(0.0_f64, f64::max);
        Ok(max_value)
    }

    async fn instant_named_metrics(
        &self,
        query: &str,
        label: &str,
        timeout: Duration,
    ) -> Result<Vec<NamedMetric>> {
        let payload = self.query("/api/v1/query", query, &[], timeout).await?;
        let result = payload
            .get("data")
            .and_then(|v| v.get("result"))
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default();

        let mut rows = result
            .iter()
            .filter_map(|row| {
                let name = row
                    .get("metric")
                    .and_then(|v| v.get(label))
                    .and_then(Value::as_str)
                    .map(str::trim)
                    .filter(|s| !s.is_empty())
                    .unwrap_or_default()
                    .to_string();
                let value = row.get("value").and_then(parse_prom_value_pair)?;
                if name.is_empty() || value <= 0.0 {
                    return None;
                }
                Some(NamedMetric { name, value })
            })
            .collect::<Vec<_>>();

        rows.sort_by(|a, b| b.value.partial_cmp(&a.value).unwrap_or(std::cmp::Ordering::Equal));
        rows.truncate(6);
        Ok(rows)
    }

    async fn component_status(&self, timeout: Duration) -> Result<Vec<ComponentStatus>> {
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
                Some(ComponentStatus {
                    job,
                    up: value >= 1.0,
                    value,
                })
            })
            .collect::<Vec<_>>();

        rows.sort_by(|a, b| a.job.cmp(&b.job));
        Ok(rows)
    }

    async fn range_series(
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

        let mut merged: BTreeMap<i64, (f64, u32)> = BTreeMap::new();
        for series in result {
            let values = series
                .get("values")
                .and_then(Value::as_array)
                .cloned()
                .unwrap_or_default();
            for pair in values {
                if let Some((ts, value)) = parse_prom_series_pair(&pair) {
                    let entry = merged.entry(ts).or_insert((0.0, 0));
                    entry.0 += value;
                    entry.1 += 1;
                }
            }
        }

        Ok(merged
            .into_iter()
            .map(|(ts, (sum, count))| MetricPoint {
                ts,
                value: if count == 0 { 0.0 } else { sum / count as f64 },
            })
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
