use anyhow::Result;
use serde::Serialize;
use serde_json::Value;

use crate::config::ClickHouseConfig;
use crate::query_helpers::{as_f64, as_u64, get_opt_str, get_str, ident, parse_rows, query_clickhouse};

const DEFAULT_WINDOW_HOURS: u16 = 24;
const MIN_WINDOW_HOURS: u16 = 1;
const MAX_WINDOW_HOURS: u16 = 168;

#[derive(Debug, Clone, Copy)]
pub struct OverviewRequest {
    pub window_hours: u16,
    pub bucket_minutes: u16,
}

impl OverviewRequest {
    pub fn from_query(hours: Option<u16>) -> Self {
        let window_hours = hours
            .unwrap_or(DEFAULT_WINDOW_HOURS)
            .clamp(MIN_WINDOW_HOURS, MAX_WINDOW_HOURS);
        let bucket_minutes = match window_hours {
            0..=24 => 1,
            25..=72 => 5,
            _ => 15,
        };
        Self {
            window_hours,
            bucket_minutes,
        }
    }
}

#[derive(Debug, Clone, Serialize)]
pub struct OverviewDashboard {
    pub window_hours: u16,
    pub bucket_minutes: u16,
    pub kpis: OverviewKpis,
    pub events_per_minute: Vec<OverviewMinutePoint>,
    pub severity_timeline: Vec<OverviewSeverityTrendPoint>,
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
pub struct OverviewSeverityTrendPoint {
    pub bucket: String,
    pub critical: u64,
    pub error: u64,
    pub warning: u64,
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

    pub async fn dashboard(&self, request: OverviewRequest, timeout: std::time::Duration) -> Result<OverviewDashboard> {
        let db = ident(&self.cfg.database)?;
        let window_hours = request.window_hours;
        let bucket_minutes = request.bucket_minutes;
        let sql_kpis = format!(
            "SELECT \
                countMerge(event_count) AS total_events_24h, \
                countMergeIf(event_count, severity = 'critical') AS critical_events_24h, \
                round(toFloat64(countMergeIf(event_count, toUInt8(severity) >= 3)) / nullIf(countMerge(event_count), 0) * 100, 2) AS error_pct_24h \
            FROM {db}.events_per_minute_agg \
            WHERE minute >= now() - INTERVAL {window_hours} HOUR \
            FORMAT JSONEachRow"
        );
        let sql_events_per_minute = format!(
            "SELECT \
                formatDateTime(bucket_ts, '%Y-%m-%dT%H:%i:%S.000Z') AS minute_iso, \
                events \
            FROM ( \
                SELECT \
                    toStartOfInterval(minute, INTERVAL {bucket_minutes} MINUTE) AS bucket_ts, \
                    countMerge(event_count) AS events \
                FROM {db}.events_per_minute_agg \
                WHERE minute >= now() - INTERVAL {window_hours} HOUR \
                GROUP BY bucket_ts \
            ) \
            ORDER BY bucket_ts \
            FORMAT JSONEachRow"
        );
        let sql_severity_timeline = format!(
            "SELECT \
                formatDateTime(bucket_ts, '%Y-%m-%dT%H:%i:%S.000Z') AS bucket_iso, \
                countMergeIf(event_count, severity = 'critical') AS critical, \
                countMergeIf(event_count, severity = 'error') AS error, \
                countMergeIf(event_count, severity = 'warning') AS warning \
            FROM ( \
                SELECT \
                    toStartOfInterval(minute, INTERVAL {bucket_minutes} MINUTE) AS bucket_ts, \
                    severity, \
                    event_count \
                FROM {db}.events_per_minute_agg \
                WHERE minute >= now() - INTERVAL {window_hours} HOUR \
            ) \
            GROUP BY bucket_ts \
            ORDER BY bucket_ts \
            FORMAT JSONEachRow"
        );
        let sql_severity = format!(
            "SELECT \
                toString(severity) AS severity_text, \
                countMerge(event_count) AS events \
            FROM {db}.events_per_minute_agg \
            WHERE minute >= now() - INTERVAL {window_hours} HOUR \
            GROUP BY severity \
            ORDER BY events DESC \
            FORMAT JSONEachRow"
        );
        let sql_sources = format!(
            "SELECT \
                source_type, \
                countMerge(event_count) AS events \
            FROM {db}.events_per_minute_agg \
            WHERE minute >= now() - INTERVAL {window_hours} HOUR \
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
            WHERE hour >= now() - INTERVAL {window_hours} HOUR \
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
            WHERE timestamp >= now() - INTERVAL {window_hours} HOUR \
              AND (severity IN ('error', 'critical', 'warning') OR source_type = 'redis') \
            ORDER BY timestamp DESC \
            LIMIT 20 \
            FORMAT JSONEachRow"
        );

        let (kpis_body, events_body, severity_timeline_body, severity_body, sources_body, ips_body, recent_body) =
            tokio::try_join!(
            query_clickhouse(&self.http, &self.cfg, &sql_kpis, timeout),
            query_clickhouse(&self.http, &self.cfg, &sql_events_per_minute, timeout),
            query_clickhouse(&self.http, &self.cfg, &sql_severity_timeline, timeout),
            query_clickhouse(&self.http, &self.cfg, &sql_severity, timeout),
            query_clickhouse(&self.http, &self.cfg, &sql_sources, timeout),
            query_clickhouse(&self.http, &self.cfg, &sql_top_ips, timeout),
            query_clickhouse(&self.http, &self.cfg, &sql_recent, timeout),
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
        let severity_timeline = parse_rows(severity_timeline_body)?
            .into_iter()
            .map(OverviewSeverityTrendPoint::from_json)
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
            window_hours,
            bucket_minutes,
            kpis,
            events_per_minute,
            severity_timeline,
            severity_breakdown,
            source_breakdown,
            top_source_ips,
            recent_security_events,
        })
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

impl OverviewSeverityTrendPoint {
    fn from_json(v: Value) -> Result<Self> {
        Ok(Self {
            bucket: get_str(&v, "bucket_iso"),
            critical: as_u64(&v, "critical"),
            error: as_u64(&v, "error"),
            warning: as_u64(&v, "warning"),
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

