use std::collections::HashMap;

use axum::extract::State;
use axum::Json;
use chrono::Utc;
use serde::Deserialize;
use serde_json::{json, Value};

use crate::models::CreateCaseRequest;
use crate::store::StoreError;
use crate::AppState;

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertmanagerWebhook {
    #[serde(default)]
    pub version: String,
    #[serde(default)]
    pub group_key: String,
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub alerts: Vec<AlertmanagerAlert>,
}

#[derive(Debug, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct AlertmanagerAlert {
    #[serde(default)]
    pub status: String,
    #[serde(default)]
    pub labels: HashMap<String, String>,
    #[serde(default)]
    pub annotations: HashMap<String, String>,
    #[serde(default)]
    pub starts_at: String,
    #[serde(default)]
    pub ends_at: String,
    #[serde(default, rename = "generatorURL")]
    pub generator_url: String,
    #[serde(default)]
    pub fingerprint: String,
}

fn severity_rank(s: &str) -> i32 {
    match s.to_lowercase().as_str() {
        "critical" => 4,
        "high" | "warning" => 3,
        "medium" => 2,
        "low" => 1,
        _ => 2,
    }
}

fn map_alert_severity(label: &str) -> String {
    match label.to_lowercase().as_str() {
        "critical" => "critical",
        "high" | "warning" => "high",
        "medium" => "medium",
        _ => "low",
    }
    .to_string()
}

fn fallback_fingerprint(alert: &AlertmanagerAlert) -> String {
    [
        alert.labels.get("alertname").map(|s| s.as_str()).unwrap_or(""),
        alert.labels.get("rule_id").map(|s| s.as_str()).unwrap_or(""),
        alert.labels.get("source_ip").map(|s| s.as_str()).unwrap_or(""),
        alert.labels.get("instance").map(|s| s.as_str()).unwrap_or(""),
    ]
    .join("|")
}

fn alert_description(alert: &AlertmanagerAlert) -> String {
    if let Some(d) = alert.annotations.get("description") {
        let trimmed = d.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    alert
        .annotations
        .get("summary")
        .map(|s| s.trim().to_string())
        .unwrap_or_default()
}

fn first_non_empty(vals: &[&str]) -> String {
    for v in vals {
        let trimmed = v.trim();
        if !trimmed.is_empty() {
            return trimmed.to_string();
        }
    }
    String::new()
}

fn priority_from_severity(sev: &str) -> i16 {
    match sev {
        "critical" => 1,
        "high" => 2,
        "medium" => 3,
        _ => 4,
    }
}

pub async fn handle_alertmanager(
    State(state): State<AppState>,
    body: String,
) -> Result<Json<Value>, crate::ApiError> {
    let payload: AlertmanagerWebhook =
        serde_json::from_str(&body).map_err(|_| crate::ApiError::bad_request("invalid json"))?;

    let min_rank = severity_rank(&state.auto_min_severity);
    let mut firing_new: i32 = 0;
    let mut firing_linked: i32 = 0;
    let mut resolved_notes: i32 = 0;
    let mut skipped: i32 = 0;

    for alert in &payload.alerts {
        let mut fp = alert.fingerprint.trim().to_string();
        if fp.is_empty() {
            fp = fallback_fingerprint(alert);
        }
        if fp.is_empty() {
            skipped += 1;
            continue;
        }

        let sev = map_alert_severity(
            alert.labels.get("severity").map(|s| s.as_str()).unwrap_or(""),
        );
        if severity_rank(&sev) < min_rank {
            skipped += 1;
            continue;
        }

        match alert.status.as_str() {
            "firing" => {
                if !state.auto_from_alerts {
                    skipped += 1;
                    continue;
                }
                match ingest_firing(&state, alert, &fp, &sev, &payload.group_key).await {
                    Ok(true) => firing_new += 1,
                    Ok(false) => firing_linked += 1,
                    Err(e) => {
                        tracing::error!(error = %e, fingerprint = %fp, "alertmanager firing");
                        return Err(crate::ApiError::internal("ingest failed"));
                    }
                }
            }
            "resolved" => match ingest_resolved(&state, &fp, alert).await {
                Ok(()) => resolved_notes += 1,
                Err(StoreError::NotFound) => skipped += 1,
                Err(e) => {
                    tracing::error!(error = %e, fingerprint = %fp, "alertmanager resolved");
                }
            },
            _ => {}
        }
    }

    Ok(Json(json!({
        "firing_new_cases": firing_new,
        "firing_linked_existing": firing_linked,
        "resolved_timeline": resolved_notes,
        "skipped": skipped,
    })))
}

fn alert_context_json(alert: &AlertmanagerAlert) -> Value {
    serde_json::to_value(&alert.labels).unwrap_or_else(|_| json!({}))
}

fn runbook_from_alert(alert: &AlertmanagerAlert) -> Option<String> {
    alert
        .annotations
        .get("runbook_url")
        .or_else(|| alert.annotations.get("runbook"))
        .map(|s| s.trim().to_string())
        .filter(|s| !s.is_empty())
}

async fn ingest_firing(
    state: &AppState,
    alert: &AlertmanagerAlert,
    fp: &str,
    sev: &str,
    group_key: &str,
) -> Result<bool, StoreError> {
    let seen_at = Utc::now();
    let labels_json = alert_context_json(alert);

    match state.store.find_active_case_by_fingerprint(fp).await {
        Ok(case_id) => {
            let desc = alert_description(alert);
            let mut rule_id = alert
                .labels
                .get("rule_id")
                .cloned()
                .unwrap_or_default();
            if rule_id.is_empty() {
                rule_id = alert
                    .labels
                    .get("alertname")
                    .cloned()
                    .unwrap_or_default();
            }
            let title = alert
                .labels
                .get("alertname")
                .cloned()
                .unwrap_or_default();
            let desc_opt = if desc.is_empty() { None } else { Some(desc.as_str()) };

            state
                .store
                .upsert_linked_alert(
                    case_id,
                    fp,
                    Some(&rule_id),
                    Some(&title),
                    Some(sev),
                    desc_opt,
                    seen_at,
                    &labels_json,
                )
                .await?;

            let _ = state
                .store
                .add_timeline(
                    case_id,
                    &state.default_actor,
                    "alert",
                    Some("Related alert fired again"),
                    json!({
                        "fingerprint": fp,
                        "rule_id": rule_id,
                        "severity": sev,
                    }),
                )
                .await;

            Ok(false)
        }
        Err(StoreError::NotFound) => {
            let title = first_non_empty(&[
                alert.annotations.get("summary").map(|s| s.as_str()).unwrap_or(""),
                alert.labels.get("alertname").map(|s| s.as_str()).unwrap_or(""),
                "Security alert",
            ]);
            let desc = alert_description(alert);
            let mut rule_id = alert
                .labels
                .get("rule_id")
                .cloned()
                .unwrap_or_default();
            if rule_id.is_empty() {
                rule_id = alert
                    .labels
                    .get("alertname")
                    .cloned()
                    .unwrap_or_default();
            }

            let runbook = runbook_from_alert(alert);
            let req = CreateCaseRequest {
                title,
                description: desc.clone(),
                severity: sev.to_string(),
                status: "new".into(),
                priority: priority_from_severity(sev),
                source: "alertmanager".into(),
                tags: vec!["auto".into(), "alertmanager".into()],
                assignee: None,
                runbook_url: runbook.clone(),
            };
            let case = state.store.create_case(req).await?;

            if let Some(ref url) = runbook {
                let _ = state
                    .store
                    .add_timeline(
                        case.id,
                        &state.default_actor,
                        "runbook",
                        Some("Runbook linked"),
                        json!({"url": url}),
                    )
                    .await;
            }

            let mut meta = json!({"fingerprint": fp});
            if !group_key.is_empty() {
                meta.as_object_mut()
                    .unwrap()
                    .insert("group_key".into(), json!(group_key));
            }
            let _ = state
                .store
                .add_timeline(
                    case.id,
                    &state.default_actor,
                    "system",
                    Some("Case opened from Alertmanager"),
                    meta,
                )
                .await;

            let alert_title = alert
                .labels
                .get("alertname")
                .cloned()
                .unwrap_or_default();
            let desc_opt = if desc.is_empty() { None } else { Some(desc.as_str()) };

            state
                .store
                .upsert_linked_alert(
                    case.id,
                    fp,
                    Some(&rule_id),
                    Some(&alert_title),
                    Some(sev),
                    desc_opt,
                    seen_at,
                    &labels_json,
                )
                .await?;

            Ok(true)
        }
        Err(e) => Err(e),
    }
}

async fn ingest_resolved(
    state: &AppState,
    fp: &str,
    alert: &AlertmanagerAlert,
) -> Result<(), StoreError> {
    let case_id = state.store.find_latest_case_by_fingerprint(fp).await?;
    let rule = alert
        .labels
        .get("alertname")
        .cloned()
        .unwrap_or_default();
    let _ = state
        .store
        .add_timeline(
            case_id,
            &state.default_actor,
            "system",
            Some("Alert resolved in Alertmanager"),
            json!({
                "fingerprint": fp,
                "rule": rule,
                "ends_at": alert.ends_at,
            }),
        )
        .await;
    Ok(())
}
