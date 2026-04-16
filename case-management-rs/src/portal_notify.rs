use std::time::Duration;

use serde_json::json;

use crate::AppState;

pub fn notify_portal(state: &AppState, topics: Vec<String>, invalidate_cases_list: bool) {
    let Some(cfg) = &state.portal_notify else {
        return;
    };
    let base = cfg.base_url.trim_end_matches('/');
    let url = format!("{base}/api/v1/realtime/notify");
    let secret = cfg.secret.clone();
    let http = state.http.clone();
    let sem = state.notify_sem.clone();
    tokio::spawn(async move {
        // Limit concurrent notification tasks to prevent unbounded spawn under load
        let _permit = match sem.acquire().await {
            Ok(p) => p,
            Err(_) => return,
        };
        let body = json!({
            "topics": topics,
            "invalidate_cases_list": invalidate_cases_list,
        });
        let res = http
            .post(&url)
            .header("Authorization", format!("Bearer {secret}"))
            .json(&body)
            .timeout(Duration::from_secs(8))
            .send()
            .await;
        if let Err(e) = res {
            tracing::warn!(error = %e, url = %url, "portal realtime notify failed");
        }
    });
}
