use axum::extract::{Path, Query, State};
use axum::http::{HeaderMap, StatusCode};
use axum::Json;
use serde::Deserialize;
use serde_json::{json, Value};
use uuid::Uuid;

use crate::models::*;
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
    let _ = state
        .store
        .add_timeline(
            case.id,
            &actor,
            "system",
            Some("Case created"),
            json!({"source": case.source}),
        )
        .await;

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
        let _ = state
            .store
            .add_timeline(
                id,
                &actor,
                "status",
                Some("Status changed"),
                json!({"from": cur.status, "to": updated.status}),
            )
            .await;
    }

    let old_assignee = cur.assignee.as_deref().unwrap_or("");
    let new_assignee = updated.assignee.as_deref().unwrap_or("");
    if old_assignee != new_assignee {
        let _ = state
            .store
            .add_timeline(
                id,
                &actor,
                "assignment",
                Some("Assignee updated"),
                json!({"from": old_assignee, "to": new_assignee}),
            )
            .await;
    }

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
    let _ = state
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
        .await;

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
        )
        .await
        .map_err(|e| {
            tracing::error!(error = %e, "link alert");
            crate::ApiError::internal("link failed")
        })?;

    let actor = actor_from_request(&headers, &state.default_actor);
    let _ = state
        .store
        .add_timeline(
            id,
            &actor,
            "alert",
            Some("Alert linked manually"),
            json!({"fingerprint": fp}),
        )
        .await;

    Ok(Json(json!({"status": "linked"})))
}
