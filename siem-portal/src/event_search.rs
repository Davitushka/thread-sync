use anyhow::{anyhow, Result};
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::Value;

use crate::config::ClickHouseConfig;

#[derive(Debug, Clone, Deserialize, Default)]
pub struct EventSearchParams {
    pub start: Option<String>,
    pub end: Option<String>,
    pub severity: Option<String>,
    pub source_type: Option<String>,
    pub host: Option<String>,
    pub source_ip: Option<String>,
    pub user_id: Option<String>,
    pub q: Option<String>,
    pub limit: Option<u32>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventSearchResponse {
    pub rows: Vec<EventRow>,
    pub meta: EventSearchMeta,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventSearchMeta {
    pub limit: u32,
    pub returned: usize,
    pub filters: EventSearchFilters,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventSearchFilters {
    pub start: String,
    pub end: String,
    pub severity: Option<String>,
    pub source_type: Option<String>,
    pub host: Option<String>,
    pub source_ip: Option<String>,
    pub user_id: Option<String>,
    pub q: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventRow {
    pub timestamp: String,
    pub event_id: String,
    pub source_type: String,
    pub event_type: String,
    pub severity: String,
    pub host: String,
    pub source_ip: Option<String>,
    pub user_id: Option<String>,
    pub action: Option<String>,
    pub status_code: Option<u16>,
    pub url_path: Option<String>,
    pub message: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventDetail {
    pub event: EventRow,
    pub duration_ms: Option<f64>,
    pub http_method: Option<String>,
    pub metadata: Value,
    pub agent_version: String,
    pub ingest_ts: String,
    pub enrich: EventEnrichment,
}

#[derive(Debug, Clone, Serialize)]
pub struct EventEnrichment {
    pub geo_country_iso: Option<String>,
    pub geo_country_name: Option<String>,
    pub geo_city: Option<String>,
    pub geo_lat: Option<f64>,
    pub geo_lon: Option<f64>,
    pub geo_asn: Option<u32>,
    pub geo_org: Option<String>,
}

#[derive(Debug, Clone, Serialize)]
pub struct EntityContextResponse {
    pub entity: EntityDescriptor,
    pub recent_events: Vec<EventRow>,
    pub metrics: EntityMetrics,
}

#[derive(Debug, Clone, Serialize)]
pub struct EntityDescriptor {
    pub kind: String,
    pub value: String,
}

#[derive(Debug, Clone, Serialize)]
pub struct EntityMetrics {
    pub total_events_24h: u64,
    pub error_events_24h: u64,
    pub top_hosts: Vec<String>,
}

#[derive(Debug, Clone)]
pub struct EventSearchService {
    http: reqwest::Client,
    cfg: ClickHouseConfig,
}

impl EventSearchService {
    pub fn new(http: reqwest::Client, cfg: ClickHouseConfig) -> Self {
        Self { http, cfg }
    }

    pub async fn search(
        &self,
        params: &EventSearchParams,
        timeout: std::time::Duration,
    ) -> Result<EventSearchResponse> {
        let filters = SearchFilters::from_params(params, &self.cfg.database)?;
        let sql = filters.build_search_sql();
        let body = self.query_json(&sql, timeout).await?;
        let rows = parse_rows(body)?
            .into_iter()
            .map(EventRow::from_json)
            .collect::<Result<Vec<_>>>()?;
        Ok(EventSearchResponse {
            meta: EventSearchMeta {
                limit: filters.limit,
                returned: rows.len(),
                filters: filters.describe(),
            },
            rows,
        })
    }

    pub async fn get_event(&self, event_id: &str, timeout: std::time::Duration) -> Result<Option<EventDetail>> {
        let event_id = sanitize_uuid(event_id).ok_or_else(|| anyhow!("invalid event id"))?;
        let sql = format!(
            "SELECT \
                formatDateTime(timestamp, '%Y-%m-%dT%H:%i:%S.%fZ') AS timestamp, \
                toString(event_id) AS event_id, \
                source_type, \
                event_type, \
                toString(severity) AS severity, \
                host, \
                ifNull(toString(source_ip), '') AS source_ip, \
                ifNull(user_id, '') AS user_id, \
                ifNull(action, '') AS action, \
                ifNull(status_code, 0) AS status_code, \
                ifNull(url_path, '') AS url_path, \
                message, \
                duration_ms, \
                ifNull(http_method, '') AS http_method, \
                mapFromArrays(mapKeys(metadata), arrayMap(v -> toString(v), mapValues(metadata))) AS metadata, \
                agent_version, \
                formatDateTime(ingest_ts, '%Y-%m-%dT%H:%i:%S.%fZ') AS ingest_ts, \
                ifNull(toString(geo_country_iso), '') AS geo_country_iso, \
                ifNull(geo_country_name, '') AS geo_country_name, \
                ifNull(geo_city, '') AS geo_city, \
                geo_lat, \
                geo_lon, \
                geo_asn, \
                ifNull(geo_org, '') AS geo_org \
            FROM {}.events \
            WHERE event_id = toUUID('{}') \
            LIMIT 1 \
            FORMAT JSONEachRow",
            ident(&self.cfg.database)?,
            event_id,
        );
        let body = self.query_json(&sql, timeout).await?;
        let mut rows = parse_rows(body)?;
        let Some(row) = rows.pop() else {
            return Ok(None);
        };
        Ok(Some(EventDetail::from_json(row)?))
    }

    pub async fn entity_context(
        &self,
        kind: &str,
        value: &str,
        timeout: std::time::Duration,
    ) -> Result<EntityContextResponse> {
        let descriptor = build_entity_descriptor(kind, value)?;
        let (where_clause, display_value) = descriptor.where_clause()?;
        let sql = format!(
            "WITH recent AS ( \
                SELECT \
                    formatDateTime(timestamp, '%Y-%m-%dT%H:%i:%S.%fZ') AS timestamp, \
                    toString(event_id) AS event_id, \
                    source_type, \
                    event_type, \
                    toString(severity) AS severity, \
                    host, \
                    ifNull(toString(source_ip), '') AS source_ip, \
                    ifNull(user_id, '') AS user_id, \
                    ifNull(action, '') AS action, \
                    ifNull(status_code, 0) AS status_code, \
                    ifNull(url_path, '') AS url_path, \
                    message \
                FROM {}.events \
                WHERE timestamp >= now() - INTERVAL 24 HOUR AND {} \
                ORDER BY timestamp DESC LIMIT 50 \
            ), metrics AS ( \
                SELECT \
                    count() AS total_events_24h, \
                    countIf(toUInt8(severity) >= 3) AS error_events_24h, \
                    groupArray(3)(host) AS top_hosts \
                FROM {}.events \
                WHERE timestamp >= now() - INTERVAL 24 HOUR AND {} \
            ) \
            SELECT \
                '{}' AS entity_kind, \
                '{}' AS entity_value, \
                (SELECT total_events_24h FROM metrics) AS total_events_24h, \
                (SELECT error_events_24h FROM metrics) AS error_events_24h, \
                (SELECT top_hosts FROM metrics) AS top_hosts, \
                groupArray(map( \
                    'timestamp', timestamp, \
                    'event_id', event_id, \
                    'source_type', source_type, \
                    'event_type', event_type, \
                    'severity', severity, \
                    'host', host, \
                    'source_ip', source_ip, \
                    'user_id', user_id, \
                    'action', action, \
                    'status_code', toString(status_code), \
                    'url_path', url_path, \
                    'message', message \
                )) AS recent_events \
            FROM recent \
            FORMAT JSONEachRow",
            ident(&self.cfg.database)?,
            where_clause,
            ident(&self.cfg.database)?,
            where_clause,
            descriptor.kind,
            escape_string(&display_value),
        );
        let body = self.query_json(&sql, timeout).await?;
        let mut rows = parse_rows(body)?;
        let row = rows.pop().unwrap_or_else(|| {
            serde_json::json!({
                "entity_kind": descriptor.kind,
                "entity_value": display_value,
                "total_events_24h": 0,
                "error_events_24h": 0,
                "top_hosts": [],
                "recent_events": [],
            })
        });
        let events = row
            .get("recent_events")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .map(EventRow::from_json)
            .collect::<Result<Vec<_>>>()?;
        let top_hosts = row
            .get("top_hosts")
            .and_then(Value::as_array)
            .cloned()
            .unwrap_or_default()
            .into_iter()
            .filter_map(|v| v.as_str().map(ToString::to_string))
            .collect::<Vec<_>>();

        Ok(EntityContextResponse {
            entity: EntityDescriptor {
                kind: descriptor.kind.to_string(),
                value: display_value,
            },
            recent_events: events,
            metrics: EntityMetrics {
                total_events_24h: as_u64(&row, "total_events_24h"),
                error_events_24h: as_u64(&row, "error_events_24h"),
                top_hosts,
            },
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

#[derive(Debug, Clone)]
struct SearchFilters {
    database: String,
    start: Option<String>,
    end: Option<String>,
    severity: Option<String>,
    source_type: Option<String>,
    host: Option<String>,
    source_ip: Option<String>,
    user_id: Option<String>,
    q: Option<String>,
    limit: u32,
}

impl SearchFilters {
    fn from_params(params: &EventSearchParams, database: &str) -> Result<Self> {
        let start = sanitize_rfc3339_like(params.start.as_deref());
        let end = sanitize_rfc3339_like(params.end.as_deref());
        let severity = sanitize_ident_like(params.severity.as_deref());
        let source_type = sanitize_ident_like(params.source_type.as_deref());
        let host = sanitize_free_text(params.host.as_deref(), 128);
        let source_ip = sanitize_ipv4(params.source_ip.as_deref());
        let user_id = sanitize_free_text(params.user_id.as_deref(), 128);
        let q = sanitize_free_text(params.q.as_deref(), 160);
        let limit = params.limit.unwrap_or(100).clamp(1, 500);
        Ok(Self {
            database: ident(database)?.to_string(),
            start,
            end,
            severity,
            source_type,
            host,
            source_ip,
            user_id,
            q,
            limit,
        })
    }

    fn describe(&self) -> EventSearchFilters {
        EventSearchFilters {
            start: self.start.clone().unwrap_or_else(|| "last_24h".to_string()),
            end: self.end.clone().unwrap_or_else(|| "now".to_string()),
            severity: self.severity.clone(),
            source_type: self.source_type.clone(),
            host: self.host.clone(),
            source_ip: self.source_ip.clone(),
            user_id: self.user_id.clone(),
            q: self.q.clone(),
        }
    }

    fn build_search_sql(&self) -> String {
        let mut where_parts = vec![if let Some(start) = &self.start {
            format!("e.timestamp >= parseDateTime64BestEffort('{}', 3, 'UTC')", start)
        } else {
            "e.timestamp >= now() - INTERVAL 24 HOUR".to_string()
        }];
        if let Some(end) = &self.end {
            where_parts.push(format!(
                "e.timestamp <= parseDateTime64BestEffort('{}', 3, 'UTC')",
                end
            ));
        }
        if let Some(severity) = &self.severity {
            where_parts.push(format!("toString(severity) = '{}'", severity));
        }
        if let Some(source_type) = &self.source_type {
            where_parts.push(format!("source_type = '{}'", source_type));
        }
        if let Some(host) = &self.host {
            where_parts.push(format!("host ILIKE '%{}%'", escape_like(host)));
        }
        if let Some(source_ip) = &self.source_ip {
            where_parts.push(format!("source_ip = toIPv4('{}')", source_ip));
        }
        if let Some(user_id) = &self.user_id {
            where_parts.push(format!("user_id ILIKE '%{}%'", escape_like(user_id)));
        }
        if let Some(q) = &self.q {
            let q = escape_like(q);
            where_parts.push(format!(
                "(message ILIKE '%{q}%' OR ifNull(url_path, '') ILIKE '%{q}%' OR ifNull(action, '') ILIKE '%{q}%')"
            ));
        }
        format!(
            "SELECT \
                formatDateTime(e.timestamp, '%Y-%m-%dT%H:%i:%S.%fZ') AS timestamp, \
                toString(event_id) AS event_id, \
                source_type, \
                event_type, \
                toString(severity) AS severity, \
                host, \
                ifNull(toString(source_ip), '') AS source_ip, \
                ifNull(user_id, '') AS user_id, \
                ifNull(action, '') AS action, \
                ifNull(status_code, 0) AS status_code, \
                ifNull(url_path, '') AS url_path, \
                message \
            FROM {}.events AS e \
            WHERE {} \
            ORDER BY e.timestamp DESC \
            LIMIT {} \
            FORMAT JSONEachRow",
            self.database,
            where_parts.join(" AND "),
            self.limit,
        )
    }
}

#[derive(Debug, Clone)]
struct EntityPredicate<'a> {
    kind: &'a str,
    value: String,
}

impl<'a> EntityPredicate<'a> {
    fn where_clause(&self) -> Result<(String, String)> {
        match self.kind {
            "ip" => Ok((
                format!("source_ip = toIPv4('{}')", self.value),
                self.value.clone(),
            )),
            "user" => Ok((
                format!("user_id ILIKE '%{}%'", escape_like(&self.value)),
                self.value.clone(),
            )),
            "host" => Ok((
                format!("host ILIKE '%{}%'", escape_like(&self.value)),
                self.value.clone(),
            )),
            _ => Err(anyhow!("unsupported entity kind")),
        }
    }
}

impl EventRow {
    fn from_json(v: Value) -> Result<Self> {
        let status_code = v
            .get("status_code")
            .and_then(Value::as_str)
            .and_then(|s| s.parse::<u16>().ok())
            .or_else(|| v.get("status_code").and_then(Value::as_u64).map(|n| n as u16));
        Ok(Self {
            timestamp: get_str(&v, "timestamp"),
            event_id: get_str(&v, "event_id"),
            source_type: get_str(&v, "source_type"),
            event_type: get_str(&v, "event_type"),
            severity: get_str(&v, "severity"),
            host: get_str(&v, "host"),
            source_ip: get_opt_str(&v, "source_ip"),
            user_id: get_opt_str(&v, "user_id"),
            action: get_opt_str(&v, "action"),
            status_code,
            url_path: get_opt_str(&v, "url_path"),
            message: get_str(&v, "message"),
        })
    }
}

impl EventDetail {
    fn from_json(v: Value) -> Result<Self> {
        let event = EventRow::from_json(v.clone())?;
        Ok(Self {
            event,
            duration_ms: v.get("duration_ms").and_then(Value::as_f64),
            http_method: get_opt_str(&v, "http_method"),
            metadata: v.get("metadata").cloned().unwrap_or_else(|| serde_json::json!({})),
            agent_version: get_str(&v, "agent_version"),
            ingest_ts: get_str(&v, "ingest_ts"),
            enrich: EventEnrichment {
                geo_country_iso: get_opt_str(&v, "geo_country_iso"),
                geo_country_name: get_opt_str(&v, "geo_country_name"),
                geo_city: get_opt_str(&v, "geo_city"),
                geo_lat: v.get("geo_lat").and_then(Value::as_f64),
                geo_lon: v.get("geo_lon").and_then(Value::as_f64),
                geo_asn: v.get("geo_asn").and_then(Value::as_u64).map(|n| n as u32),
                geo_org: get_opt_str(&v, "geo_org"),
            },
        })
    }
}

fn build_entity_descriptor<'a>(kind: &'a str, value: &str) -> Result<EntityPredicate<'a>> {
    let kind = kind.trim().to_ascii_lowercase();
    let kind = match kind.as_str() {
        "ip" | "source_ip" => "ip",
        "user" | "user_id" => "user",
        "host" => "host",
        _ => return Err(anyhow!("unsupported entity kind")),
    };
    let value = match kind {
        "ip" => sanitize_ipv4(Some(value)).ok_or_else(|| anyhow!("invalid ip"))?,
        _ => sanitize_free_text(Some(value), 160).ok_or_else(|| anyhow!("invalid value"))?,
    };
    Ok(EntityPredicate { kind, value })
}

fn sanitize_rfc3339_like(input: Option<&str>) -> Option<String> {
    let raw = input?.trim();
    if raw.is_empty() || raw.len() > 40 {
        return None;
    }
    if raw
        .chars()
        .all(|c| c.is_ascii_digit() || matches!(c, '-' | ':' | 'T' | 'Z' | '.' | '+' | ' '))
    {
        Some(raw.to_string())
    } else {
        None
    }
}

fn sanitize_ident_like(input: Option<&str>) -> Option<String> {
    let raw = input?.trim();
    if raw.is_empty() || raw.len() > 64 {
        return None;
    }
    if raw
        .chars()
        .all(|c| c.is_ascii_alphanumeric() || matches!(c, '_' | '-' | '.'))
    {
        Some(raw.to_ascii_lowercase())
    } else {
        None
    }
}

fn sanitize_ipv4(input: Option<&str>) -> Option<String> {
    let raw = input?.trim();
    if raw.is_empty() || raw.len() > 15 {
        return None;
    }
    if !raw.chars().all(|c| c.is_ascii_digit() || c == '.') {
        return None;
    }
    let octets = raw.split('.').collect::<Vec<_>>();
    if octets.len() != 4 {
        return None;
    }
    if octets
        .iter()
        .all(|o| !o.is_empty() && o.parse::<u8>().is_ok())
    {
        Some(raw.to_string())
    } else {
        None
    }
}

fn sanitize_uuid(input: &str) -> Option<String> {
    let raw = input.trim();
    if raw.len() != 36 {
        return None;
    }
    if raw
        .chars()
        .all(|c| c.is_ascii_hexdigit() || c == '-')
    {
        Some(raw.to_ascii_lowercase())
    } else {
        None
    }
}

fn sanitize_free_text(input: Option<&str>, max_len: usize) -> Option<String> {
    let raw = input?.trim();
    if raw.is_empty() {
        return None;
    }
    let filtered = raw
        .chars()
        .filter(|c| !matches!(c, '\'' | '"' | ';' | '\\'))
        .take(max_len)
        .collect::<String>();
    if filtered.trim().is_empty() {
        None
    } else {
        Some(filtered.trim().to_string())
    }
}

fn escape_string(v: &str) -> String {
    v.replace('\\', "\\\\").replace('\'', "\\'")
}

fn escape_like(v: &str) -> String {
    escape_string(v).replace('%', "\\%").replace('_', "\\_")
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

fn as_u64(v: &Value, key: &str) -> u64 {
    v.get(key)
        .and_then(Value::as_u64)
        .or_else(|| v.get(key).and_then(Value::as_str).and_then(|s| s.parse::<u64>().ok()))
        .unwrap_or(0)
}

