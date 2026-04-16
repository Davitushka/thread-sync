use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use std::str::FromStr;
use uuid::Uuid;

use crate::models::*;
use crate::portal_notify;
use crate::store::StoreError;
use crate::validate::{validate_resolution, validate_severity, validate_status};
use crate::AppState;

#[derive(Debug, Deserialize)]
pub struct ListParams {
    pub status: Option<String>,
    pub severity: Option<String>,
    pub assignee: Option<String>,
    pub q: Option<String>,
    pub limit: Option<i32>,
    pub offset: Option<i32>,
}

fn actor_from_request(headers: &HeaderMap, fallback: &str) -> String {
    headers
        .get("X-SOC-Actor")
        .and_then(|v| v.to_str().ok())
        .map(|s| s.trim())
        .filter(|s| !s.is_empty())
        .map(|s| s.to_string())
        .unwrap_or_else(|| fallback.to_string())
}

pub async fn health() -> Json<Value> {
    Json(json!({"status": "ok"}))
}

pub async fn list_cases(
    State(state): State<AppState>,
    Query(params): Query<ListParams>,
) -> Result<Json<Value>, crate::ApiError> {
    let filter = crate::store::ListFilter {
        status: params.status.unwrap_or_default(),
        severity: params.severity.unwrap_or_default(),
        assignee: params.assignee.unwrap_or_default(),
        query: params.q.unwrap_or_default(),
        limit: params.limit.unwrap_or(0),
        offset: params.offset.unwrap_or(0),
    };
    let (cases, total) = state.store.list_cases(filter).await.map_err(|e| {
        tracing::error!(error = %e, "list cases");
        crate::ApiError::internal("list failed")
    })?;
    Ok(Json(json!({"cases": cases, "total": total})))
}

pub async fn create_case(
    State(state): State<AppState>,
    headers: HeaderMap,
    body: String,
) -> Result<(StatusCode, Json<Case>), crate::ApiError> {
    let mut req: CreateCaseRequest =
        serde_json::from_str(&body).map_err(|_| crate::ApiError::bad_request("invalid json"))?;

    if req.title.trim().is_empty() {
        return Err(crate::ApiError::bad_request("title required"));
    }
    if req.severity.is_empty() {
        req.severity = "medium".into();
    }
    if req.status.is_empty() {
        req.status = "new".into();
    }
    validate_severity(&req.severity).map_err(|_| crate::ApiError::bad_request("invalid severity"))?;
    validate_status(&req.status).map_err(|_| crate::ApiError::bad_request("invalid status"))?;
    if req.priority == 0 {
        req.priority = 2;
    }
    if req.source.is_empty() {
        req.source = "api".into();
    }

    let case = state.store.create_case(req).await.map_err(|e| {
        tracing::error!(error = %e, "create case");
        crate::ApiError::internal("create failed")
    })?;

    let actor = actor_from_request(&headers, &state.default_actor);
    if let Err(e) = state
        .store
        .add_timeline(
            case.id,
            &actor,
            "system",
            Some("Case created"),
            json!({"source": case.source}),
        )
        .await
    {
        tracing::warn!(error = %e, case_id = %case.id, "failed to add creation timeline");
    }

    portal_notify::notify_portal(
        &state,
        vec![
            format!("case.detail:{}", case.id),
            format!("case.investigate:{}", case.id),
        ],
        true,
    );

    Ok((StatusCode::CREATED, Json(case)))
}

pub async fn get_case(
    State(state): State<AppState>,
    Path(id_str): Path<String>,
) -> Result<Json<CaseDetail>, crate::ApiError> {
    let id = Uuid::parse_str(&id_str).map_err(|_| crate::ApiError::bad_request("invalid id"))?;
    let detail = match state.store.get_case_detail(id).await {
        Ok(d) => d,
        Err(StoreError::NotFound) => return Err(crate::ApiError::not_found("not found")),
        Err(e) => {
            tracing::error!(error = %e, "get case");
            return Err(crate::ApiError::internal("get failed"));
        }
    };
    Ok(Json(detail))
}

pub async fn patch_case(
    State(state): State<AppState>,
    Path(id_str): Path<String>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<Case>, crate::ApiError> {
    let id = Uuid::parse_str(&id_str).map_err(|_| crate::ApiError::bad_request("invalid id"))?;
    let req: PatchCaseRequest =
        serde_json::from_str(&body).map_err(|_| crate::ApiError::bad_request("invalid json"))?;

    if let Some(ref sev) = req.severity {
        validate_severity(sev).map_err(|_| crate::ApiError::bad_request("invalid severity"))?;
    }
    if let Some(ref st) = req.status {
        validate_status(st).map_err(|_| crate::ApiError::bad_request("invalid status"))?;
    }
    if let Some(ref res) = req.resolution {
        if !res.is_empty() {
            validate_resolution(res)
                .map_err(|_| crate::ApiError::bad_request("invalid resolution"))?;
        }
    }

    let cur = match state.store.get_case(id).await {
        Ok(c) => c,
        Err(StoreError::NotFound) => return Err(crate::ApiError::not_found("not found")),
        Err(_) => return Err(crate::ApiError::internal("get failed")),
    };

    let updated = state.store.patch_case(id, req).await.map_err(|e| {
        tracing::error!(error = %e, "patch case");
        crate::ApiError::internal("update failed")
    })?;

    let actor = actor_from_request(&headers, &state.default_actor);

    if cur.status != updated.status {
        if let Err(e) = state
            .store
            .add_timeline(
                id,
                &actor,
                "status",
                Some("Status changed"),
                json!({"from": cur.status, "to": updated.status}),
            )
            .await
        {
            tracing::warn!(error = %e, case_id = %id, "failed to add status timeline");
        }
    }

    let old_assignee = cur.assignee.as_deref().unwrap_or("");
    let new_assignee = updated.assignee.as_deref().unwrap_or("");
    if old_assignee != new_assignee {
        if let Err(e) = state
            .store
            .add_timeline(
                id,
                &actor,
                "assignment",
                Some("Assignee updated"),
                json!({"from": old_assignee, "to": new_assignee}),
            )
            .await
        {
            tracing::warn!(error = %e, case_id = %id, "failed to add assignment timeline");
        }
    }

    portal_notify::notify_portal(
        &state,
        vec![
            format!("case.detail:{}", id),
            format!("case.investigate:{}", id),
        ],
        true,
    );

    Ok(Json(updated))
}

pub async fn add_timeline(
    State(state): State<AppState>,
    Path(id_str): Path<String>,
    headers: HeaderMap,
    body: String,
) -> Result<(StatusCode, Json<TimelineEntry>), crate::ApiError> {
    let id = Uuid::parse_str(&id_str).map_err(|_| crate::ApiError::bad_request("invalid id"))?;
    let req: TimelineCreateRequest =
        serde_json::from_str(&body).map_err(|_| crate::ApiError::bad_request("invalid json"))?;

    if req.body.trim().is_empty() {
        return Err(crate::ApiError::bad_request("body required"));
    }

    match state.store.get_case(id).await {
        Ok(_) => {}
        Err(StoreError::NotFound) => return Err(crate::ApiError::not_found("not found")),
        Err(_) => return Err(crate::ApiError::internal("get failed")),
    }

    let actor = actor_from_request(&headers, &state.default_actor);
    let entry = state
        .store
        .add_timeline(id, &actor, "comment", Some(&req.body), json!({}))
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "timeline");
            crate::ApiError::internal("insert failed")
        })?;

    portal_notify::notify_portal(
        &state,
        vec![
            format!("case.detail:{}", id),
            format!("case.investigate:{}", id),
        ],
        false,
    );

    Ok((StatusCode::CREATED, Json(entry)))
}

pub async fn link_event(
    State(state): State<AppState>,
    Path(id_str): Path<String>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<Value>, crate::ApiError> {
    let id = Uuid::parse_str(&id_str).map_err(|_| crate::ApiError::bad_request("invalid id"))?;
    let req: LinkEventRequest =
        serde_json::from_str(&body).map_err(|_| crate::ApiError::bad_request("invalid json"))?;

    if req.event_id.is_nil() {
        return Err(crate::ApiError::bad_request("event_id required"));
    }

    match state.store.get_case(id).await {
        Ok(_) => {}
        Err(StoreError::NotFound) => return Err(crate::ApiError::not_found("not found")),
        Err(_) => return Err(crate::ApiError::internal("get failed")),
    }

    state
        .store
        .link_event(id, req.event_id, req.note.as_deref())
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "link event");
            crate::ApiError::internal("link failed")
        })?;

    let actor = actor_from_request(&headers, &state.default_actor);
    if let Err(e) = state
        .store
        .add_timeline(
            id,
            &actor,
            "event",
            Some("Event linked to case"),
            json!({
                "event_id": req.event_id.to_string(),
                "explore_url": format!("{}/explore", state.grafana_base_url),
            }),
        )
        .await
    {
        tracing::warn!(error = %e, case_id = %id, "failed to add event link timeline");
    }

    portal_notify::notify_portal(
        &state,
        vec![
            format!("case.detail:{}", id),
            format!("case.investigate:{}", id),
        ],
        false,
    );

    Ok(Json(json!({"status": "linked"})))
}

pub async fn link_alert(
    State(state): State<AppState>,
    Path(id_str): Path<String>,
    headers: HeaderMap,
    body: String,
) -> Result<Json<Value>, crate::ApiError> {
    let id = Uuid::parse_str(&id_str).map_err(|_| crate::ApiError::bad_request("invalid id"))?;
    let req: LinkAlertRequest =
        serde_json::from_str(&body).map_err(|_| crate::ApiError::bad_request("invalid json"))?;

    let fp = req.fingerprint.trim().to_string();
    if fp.is_empty() {
        return Err(crate::ApiError::bad_request("fingerprint required"));
    }

    match state.store.get_case(id).await {
        Ok(_) => {}
        Err(StoreError::NotFound) => return Err(crate::ApiError::not_found("not found")),
        Err(_) => return Err(crate::ApiError::internal("get failed")),
    }

    let now = chrono::Utc::now();
    let ctx = if req.context.is_null() {
        json!({})
    } else {
        req.context.clone()
    };
    state
        .store
        .upsert_linked_alert(
            id,
            &fp,
            req.rule_id.as_deref(),
            req.rule_title.as_deref(),
            req.severity.as_deref(),
            req.description.as_deref(),
            now,
            &ctx,
        )
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "link alert");
            crate::ApiError::internal("link failed")
        })?;

    let actor = actor_from_request(&headers, &state.default_actor);
    if let Err(e) = state
        .store
        .add_timeline(
            id,
            &actor,
            "alert",
            Some("Alert linked manually"),
            json!({"fingerprint": fp}),
        )
        .await
    {
        tracing::warn!(error = %e, case_id = %id, "failed to add alert link timeline");
    }

    portal_notify::notify_portal(
        &state,
        vec![
            format!("case.detail:{}", id),
            format!("case.investigate:{}", id),
        ],
        false,
    );

    Ok(Json(json!({"status": "linked"})))
}

/// Validates an IP address string using standard parsing.
/// Returns the validated IP string if it parses correctly, None otherwise.
fn sanitize_ip_for_sql(ip: &str) -> Option<String> {
    let ip = ip.trim();
    if ip.is_empty() {
        return None;
    }
    std::net::IpAddr::from_str(ip)
        .ok()
        .map(|_| ip.to_string())
}

/// Сводка для расследования: ссылки Grafana и шаблоны SQL по контексту связанных алертов.
pub async fn investigate_case(
    State(state): State<AppState>,
    Path(id_str): Path<String>,
) -> Result<Json<Value>, crate::ApiError> {
    let id = Uuid::parse_str(&id_str).map_err(|_| crate::ApiError::bad_request("invalid id"))?;
    let detail = match state.store.get_case_detail(id).await {
        Ok(d) => d,
        Err(StoreError::NotFound) => return Err(crate::ApiError::not_found("not found")),
        Err(e) => {
            tracing::error!(error = %e, "investigate");
            return Err(crate::ApiError::internal("load failed"));
        }
    };

    let g = state.grafana_base_url.trim_end_matches('/');
    let explore_ch = format!(
        "{}/explore?schemaVersion=1&panes=%7B%22siem%22%3A%7B%22datasource%22%3A%22clickhouse-siem%22%2C%22queries%22%3A%5B%7B%22refId%22%3A%22A%22%2C%22queryType%22%3A%22sql%22%2C%22rawSql%22%3A%22SELECT%20%2A%20FROM%20siem.events%20WHERE%20timestamp%20%3E%3D%20now()%20-%20INTERVAL%201%20HOUR%20ORDER%20BY%20timestamp%20DESC%20LIMIT%2050%22%7D%5D%7D%7D",
        g
    );
    let explore_loki = format!("{}/explore?schemaVersion=1&panes=%7B%22l%22%3A%7B%22datasource%22%3A%22loki-siem%22%7D%7D", g);
    let overview = format!("{}/d/siem-overview/siem-lite-obzor", g);
    let data_quality = format!("{}/d/siem-data-quality/siem-lite-doverie-k-dannym", g);

    let mut suggested_queries: Vec<Value> = Vec::new();
    for alert in &detail.linked_alerts {
        let ctx = &alert.context;
        let ip_raw = ctx
            .get("source_ip")
            .and_then(|v| v.as_str())
            .unwrap_or("");
        if let Some(ip) = sanitize_ip_for_sql(ip_raw) {
            suggested_queries.push(json!({
                "title": format!("События с source_ip = {} (24 ч)", ip),
                "sql": format!(
                    "SELECT formatDateTime(timestamp, '%Y-%m-%d %H:%i:%s') AS t, toString(severity) AS sev, source_type, left(message, 120) AS msg, url_path \
                     FROM siem.events WHERE timestamp >= now() - INTERVAL 24 HOUR AND source_ip = toIPv4('{ip}') \
                     ORDER BY timestamp DESC LIMIT 300",
                ),
            }));
        }
        if let Some(rid) = alert.rule_id.as_deref().filter(|s| !s.is_empty()) {
            let safe_rid: String = rid
                .chars()
                .filter(|c| c.is_alphanumeric() || *c == '_' || *c == '-')
                .take(64)
                .collect();
            if !safe_rid.is_empty() {
                suggested_queries.push(json!({
                    "title": format!("События с rule_id в metadata или в message (24 ч): {}", safe_rid),
                    "sql": format!(
                        "SELECT formatDateTime(timestamp, '%Y-%m-%d %H:%i:%s') AS t, source_type, left(message, 100) AS msg \
                         FROM siem.events WHERE timestamp >= now() - INTERVAL 24 HOUR \
                           AND (metadata['rule_id'] = '{safe_rid}' OR message ILIKE concat('%', '{safe_rid}', '%')) \
                         ORDER BY timestamp DESC LIMIT 150",
                    ),
                }));
            }
        }
    }

    if suggested_queries.is_empty() {
        suggested_queries.push(json!({
            "title": "Последние события SIEM (24 ч)",
            "sql": "SELECT formatDateTime(timestamp, '%Y-%m-%d %H:%i:%s') AS t, toString(severity) AS sev, source_type, left(message, 100) AS msg \
                    FROM siem.events WHERE timestamp >= now() - INTERVAL 24 HOUR ORDER BY timestamp DESC LIMIT 100",
        }));
    }

    Ok(Json(json!({
        "case_id": id,
        "display_key": detail.case.display_key,
        "due_at": detail.case.due_at,
        "acknowledged_at": detail.case.acknowledged_at,
        "runbook_url": detail.case.runbook_url,
        "grafana": {
            "siem_overview": overview,
            "explore_clickhouse_preset": explore_ch,
            "explore_loki": explore_loki,
            "data_quality_dashboard": data_quality,
        },
        "suggested_clickhouse_queries": suggested_queries,
        "process": {
            "status_workflow": ["new", "triaged", "investigating", "contained", "resolved", "closed"],
            "sla_hint": "due_at задаётся при создании по severity (critical 4h, high 8h, medium 24h, low 72h).",
        },
    })))
}
