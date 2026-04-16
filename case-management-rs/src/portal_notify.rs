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
    tokio::spawn(async move {
        let body = json!({
            "secret": secret,
            "topics": topics,
            "invalidate_cases_list": invalidate_cases_list,
        });
        let res = http
            .post(&url)
            .json(&body)
            .timeout(Duration::from_secs(8))
            .send()
            .await;
        if let Err(e) = res {
            tracing::debug!(error = %e, url = %url, "portal realtime notify");
        }
    });
}
