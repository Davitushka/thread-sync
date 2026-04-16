//! WebSocket realtime: topic subscriptions, server-side polling of upstreams,
//! push snapshots when payload fingerprint changes.

use std::collections::{HashMap, HashSet};
use std::hash::{Hash, Hasher};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use axum::extract::ws::{Message, WebSocket, WebSocketUpgrade};
use axum::extract::State;
use axum::response::IntoResponse;
use reqwest::Url;
use serde::{Deserialize, Serialize};
use serde_json::Value;
use tokio::sync::{broadcast, RwLock};

use crate::config::RealtimePolicy;
use crate::data_quality::DataQualityRequest;
use crate::event_search::EventSearchParams;
use crate::handlers::{join_case_management, CasesQuery};
use crate::infrastructure::InfrastructureRequest;
use crate::operations::OperationsRequest;
use crate::overview::OverviewRequest;
use crate::AppState;

const BROADCAST_CAP: usize = 1024;

fn now_ms() -> i64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_millis() as i64
}

fn fingerprint(value: &Value) -> u64 {
    let bytes = serde_json::to_vec(value).unwrap_or_default();
    let mut h = std::collections::hash_map::DefaultHasher::new();
    bytes.hash(&mut h);
    h.finish()
}

async fn http_get_json(client: &reqwest::Client, url: Url, timeout: Duration) -> Result<Value, String> {
    let response = client
        .get(url.clone())
        .timeout(timeout)
        .send()
        .await
        .map_err(|e| format!("http get {url}: {e}"))?;
    if !response.status().is_success() {
        return Err(format!("upstream {} for {}", response.status(), url));
    }
    response.json().await.map_err(|e| format!("json {url}: {e}"))
}

/// Fetch JSON for a canonical topic string (same keys the browser sends on subscribe).
pub async fn fetch_snapshot(state: &AppState, topic: &str) -> Result<Value, String> {
    let timeout = state.cfg.http_timeout;

    if topic == "ui.config" {
        return Ok(crate::handlers::ui_config_json(state));
    }
    if topic == "stack.status" {
        return Ok(crate::handlers::stack_status_json(state).await);
    }

    if let Some(hs) = topic.strip_prefix("overview:h:") {
        let hours: Option<u16> = hs.parse().ok();
        let payload = state
            .overview
            .dashboard(OverviewRequest::from_query(hours), timeout)
            .await
            .map_err(|e| e.to_string())?;
        return serde_json::to_value(payload).map_err(|e| e.to_string());
    }

    if let Some(hs) = topic.strip_prefix("infrastructure:h:") {
        let hours: Option<u16> = hs.parse().ok();
        let payload = state
            .infrastructure
            .dashboard(InfrastructureRequest::from_query(hours), timeout)
            .await
            .map_err(|e| e.to_string())?;
        return serde_json::to_value(payload).map_err(|e| e.to_string());
    }

    if let Some(hs) = topic.strip_prefix("operations:h:") {
        let hours: Option<u16> = hs.parse().ok();
        let payload = state
            .operations
            .dashboard(OperationsRequest::from_query(hours), timeout)
            .await
            .map_err(|e| e.to_string())?;
        return serde_json::to_value(payload).map_err(|e| e.to_string());
    }

    if let Some(hs) = topic.strip_prefix("data_quality:h:") {
        let hours: Option<u16> = hs.parse().ok();
        let payload = state
            .data_quality
            .dashboard(DataQualityRequest::from_query(hours), timeout)
            .await
            .map_err(|e| e.to_string())?;
        return serde_json::to_value(payload).map_err(|e| e.to_string());
    }

    if topic == "alerts.overview" {
        let payload = state.alerts.overview(timeout).await.map_err(|e| e.to_string())?;
        return serde_json::to_value(payload).map_err(|e| e.to_string());
    }

    if topic == "detections.overview" {
        let payload = state.detections.overview(timeout).await.map_err(|e| e.to_string())?;
        return serde_json::to_value(payload).map_err(|e| e.to_string());
    }

    if topic == "correlator.stats" {
        let base: Url = state
            .cfg
            .correlator
            .parse()
            .map_err(|e| format!("correlator url: {e}"))?;
        let url = base
            .join("/api/v1/stats")
            .map_err(|e| format!("correlator join: {e}"))?;
        return http_get_json(&state.http, url, timeout).await;
    }

    if topic == "correlator.rules" {
        let base: Url = state
            .cfg
            .correlator
            .parse()
            .map_err(|e| format!("correlator url: {e}"))?;
        let url = base
            .join("/api/v1/rules")
            .map_err(|e| format!("correlator join: {e}"))?;
        return http_get_json(&state.http, url, timeout).await;
    }

    if topic == "alertmanager.alerts" {
        let url = format!("{}/api/v2/alerts", state.cfg.alertmanager);
        let u: Url = url.parse().map_err(|e| format!("alertmanager url: {e}"))?;
        return http_get_json(&state.http, u, timeout).await;
    }

    if topic == "cases.list" || topic.starts_with("cases.list?") {
        let qs = topic.strip_prefix("cases.list?").unwrap_or("");
        let q: CasesQuery = if qs.is_empty() {
            CasesQuery::default()
        } else {
            serde_urlencoded::from_str(qs).map_err(|e| format!("cases query: {e}"))?
        };
        let mut url = join_case_management(state, "/api/v1/cases").map_err(|_| "case management url".to_string())?;
        {
            let mut pairs = url.query_pairs_mut();
            if let Some(s) = &q.status {
                pairs.append_pair("status", s);
            }
            if let Some(s) = &q.severity {
                pairs.append_pair("severity", s);
            }
            if let Some(s) = &q.assignee {
                pairs.append_pair("assignee", s);
            }
            if let Some(l) = q.limit {
                pairs.append_pair("limit", &l.to_string());
            }
            if let Some(o) = q.offset {
                pairs.append_pair("offset", &o.to_string());
            }
            if let Some(s) = &q.q {
                pairs.append_pair("q", s);
            }
        }
        return http_get_json(&state.http, url, timeout).await;
    }

    if let Some(id) = topic.strip_prefix("case.detail:") {
        if id.is_empty() {
            return Err("case.detail empty id".into());
        }
        let url = join_case_management(state, &format!("/api/v1/cases/{id}"))
            .map_err(|_| "case management url".to_string())?;
        return http_get_json(&state.http, url, timeout).await;
    }

    if let Some(id) = topic.strip_prefix("case.investigate:") {
        if id.is_empty() {
            return Err("case.investigate empty id".into());
        }
        let url = join_case_management(state, &format!("/api/v1/cases/{id}/investigate"))
            .map_err(|_| "case management url".to_string())?;
        return http_get_json(&state.http, url, timeout).await;
    }

    if topic == "events.search" || topic.starts_with("events.search?") {
        let qs = topic.strip_prefix("events.search?").unwrap_or("");
        let params: EventSearchParams = if qs.is_empty() {
            EventSearchParams::default()
        } else {
            serde_urlencoded::from_str(qs).map_err(|e| format!("events params: {e}"))?
        };
        let payload = state
            .event_search
            .search(&params, timeout)
            .await
            .map_err(|e| e.to_string())?;
        return serde_json::to_value(payload).map_err(|e| e.to_string());
    }

    if let Some(id) = topic.strip_prefix("event.detail:") {
        if id.is_empty() {
            return Err("event.detail empty id".into());
        }
        let row = state
            .event_search
            .get_event(id, timeout)
            .await
            .map_err(|e| e.to_string())?;
        return match row {
            Some(payload) => serde_json::to_value(payload).map_err(|e| e.to_string()),
            None => Err("event not found".into()),
        };
    }

    if let Some(rest) = topic.strip_prefix("entity.context:") {
        let v: Value = serde_json::from_str(rest).map_err(|e| format!("entity.context json: {e}"))?;
        let kind = v
            .get("kind")
            .and_then(|x| x.as_str())
            .ok_or_else(|| "entity.context: missing kind".to_string())?;
        let value = v
            .get("value")
            .and_then(|x| x.as_str())
            .ok_or_else(|| "entity.context: missing value".to_string())?;
        let payload = state
            .event_search
            .entity_context(kind, value, timeout)
            .await
            .map_err(|e| e.to_string())?;
        return serde_json::to_value(payload).map_err(|e| e.to_string());
    }

    Err(format!("unknown realtime topic: {topic}"))
}

#[derive(Debug, Clone, Serialize)]
#[serde(tag = "type", rename_all = "snake_case")]
pub enum ServerMsg {
    Welcome {
        protocol: u32,
        poll_ms: u64,
        server: &'static str,
    },
    Snapshot {
        topic: String,
        at_ms: i64,
        data: Value,
    },
    Error {
        topic: String,
        message: String,
    },
    Pong {
        nonce: Option<u64>,
    },
}

#[derive(Debug, Deserialize)]
#[serde(tag = "type", rename_all = "snake_case")]
enum ClientMsg {
    Subscribe { topics: Vec<String> },
    Unsubscribe { topics: Vec<String> },
    Ping { nonce: Option<u64> },
}

struct RealtimeInner {
    tx: broadcast::Sender<ServerMsg>,
    ref_counts: RwLock<HashMap<String, u32>>,
    last_fp: RwLock<HashMap<String, u64>>,
    policy: RealtimePolicy,
}

#[derive(Clone)]
pub struct RealtimeHub {
    inner: Arc<RealtimeInner>,
}

impl RealtimeHub {
    pub fn new(policy: RealtimePolicy) -> Self {
        let (tx, _rx) = broadcast::channel(BROADCAST_CAP);
        Self {
            inner: Arc::new(RealtimeInner {
                tx,
                ref_counts: RwLock::new(HashMap::new()),
                last_fp: RwLock::new(HashMap::new()),
                policy,
            }),
        }
    }

    pub fn poll_ms(&self) -> u64 {
        self.inner.policy.default_ms
    }

    pub fn subscribe(&self) -> broadcast::Receiver<ServerMsg> {
        self.inner.tx.subscribe()
    }

    fn send_all(&self, msg: ServerMsg) {
        let _ = self.inner.tx.send(msg);
    }

    async fn register_topic(&self, topic: &str) {
        let mut w = self.inner.ref_counts.write().await;
        *w.entry(topic.to_string()).or_insert(0) += 1;
    }

    async fn unregister_topic(&self, topic: &str) {
        let mut cleared = false;
        {
            let mut w = self.inner.ref_counts.write().await;
            if let Some(v) = w.get_mut(topic) {
                *v = v.saturating_sub(1);
                if *v == 0 {
                    w.remove(topic);
                    cleared = true;
                }
            }
        }
        if cleared {
            let mut fp = self.inner.last_fp.write().await;
            fp.remove(topic);
        }
    }

    async fn active_topics(&self) -> Vec<String> {
        self.inner.ref_counts.read().await.keys().cloned().collect()
    }

    /// Returns true if this fingerprint is new for the topic (engine should broadcast).
    async fn consume_fp_if_new(&self, topic: &str, fp: u64) -> bool {
        let mut w = self.inner.last_fp.write().await;
        if w.get(topic).copied() == Some(fp) {
            return false;
        }
        w.insert(topic.to_string(), fp);
        true
    }

    /// After an immediate push to one client, record fp so the engine does not duplicate.
    async fn prime_fp(&self, topic: &str, fp: u64) {
        let mut w = self.inner.last_fp.write().await;
        w.insert(topic.to_string(), fp);
    }
}

pub fn spawn_engine(state: AppState) {
    let hub = state.realtime.clone();
    let policy = state.cfg.realtime_policy.clone();
    tokio::spawn(async move {
        let engine_tick = Duration::from_millis(200);
        let mut tick = tokio::time::interval(engine_tick);
        tick.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Delay);
        let mut last_poll: HashMap<String, Instant> = HashMap::new();
        loop {
            tick.tick().await;
            let topics = hub.active_topics().await;
            if topics.is_empty() {
                last_poll.clear();
                continue;
            }
            let active: HashSet<String> = topics.iter().cloned().collect();
            last_poll.retain(|k, _| active.contains(k));
            let now = Instant::now();
            for topic in topics {
                let period = Duration::from_millis(policy.poll_ms_for_topic(&topic).max(500));
                let do_fetch = last_poll
                    .get(&topic)
                    .map(|t0| now.duration_since(*t0) >= period)
                    .unwrap_or(true);
                if !do_fetch {
                    continue;
                }
                last_poll.insert(topic.clone(), now);
                let t0 = Instant::now();
                match fetch_snapshot(&state, &topic).await {
                    Ok(v) => {
                        let fp = fingerprint(&v);
                        if hub.consume_fp_if_new(&topic, fp).await {
                            tracing::debug!(topic = %topic, ms = t0.elapsed().as_millis() as u64, "realtime snapshot");
                            hub.send_all(ServerMsg::Snapshot {
                                topic: topic.clone(),
                                at_ms: now_ms(),
                                data: v,
                            });
                        }
                    }
                    Err(e) => {
                        tracing::warn!(topic = %topic, error = %e, "realtime fetch failed");
                    }
                }
            }
        }
    });
}

async fn push_immediate(socket: &mut WebSocket, state: &AppState, hub: &RealtimeHub, topic: &str) {
    match fetch_snapshot(state, topic).await {
        Ok(v) => {
            let fp = fingerprint(&v);
            hub.prime_fp(topic, fp).await;
            let msg = ServerMsg::Snapshot {
                topic: topic.to_string(),
                at_ms: now_ms(),
                data: v,
            };
            if let Ok(text) = serde_json::to_string(&msg) {
                let _ = socket.send(Message::Text(text.into())).await;
            }
        }
        Err(e) => {
            let msg = ServerMsg::Error {
                topic: topic.to_string(),
                message: e,
            };
            if let Ok(text) = serde_json::to_string(&msg) {
                let _ = socket.send(Message::Text(text.into())).await;
            }
        }
    }
}

async fn handle_socket(mut socket: WebSocket, state: AppState) {
    let hub = state.realtime.clone();
    let mut rx = hub.subscribe();
    let mut local: HashSet<String> = HashSet::new();

    let welcome = ServerMsg::Welcome {
        protocol: 1,
        poll_ms: hub.poll_ms(),
        server: "siem-portal",
    };
    if let Ok(text) = serde_json::to_string(&welcome) {
        if socket.send(Message::Text(text.into())).await.is_err() {
            return;
        }
    }

    loop {
        tokio::select! {
            inc = socket.recv() => {
                match inc {
                    None => break,
                    Some(Ok(Message::Text(t))) => {
                        let Ok(msg) = serde_json::from_str::<ClientMsg>(&t) else { continue };
                        match msg {
                            ClientMsg::Ping { nonce } => {
                                let pong = ServerMsg::Pong { nonce };
                                if let Ok(txt) = serde_json::to_string(&pong) {
                                    let _ = socket.send(Message::Text(txt.into())).await;
                                }
                            }
                            ClientMsg::Subscribe { topics } => {
                                for t in topics.into_iter().filter(|s| !s.is_empty()) {
                                    if local.insert(t.clone()) {
                                        hub.register_topic(&t).await;
                                        push_immediate(&mut socket, &state, &hub, &t).await;
                                    }
                                }
                            }
                            ClientMsg::Unsubscribe { topics } => {
                                for t in topics.into_iter().filter(|s| !s.is_empty()) {
                                    if local.remove(&t) {
                                        hub.unregister_topic(&t).await;
                                    }
                                }
                            }
                        }
                    }
                    Some(Ok(Message::Ping(payload))) => {
                        let _ = socket.send(Message::Pong(payload)).await;
                    }
                    Some(Ok(Message::Close(_))) => break,
                    Some(Ok(_)) => {}
                    Some(Err(_)) => break,
                }
            }
            push = rx.recv() => {
                match push {
                    Ok(ServerMsg::Snapshot { topic, at_ms, data }) => {
                        if !local.contains(&topic) { continue; }
                        let msg = ServerMsg::Snapshot { topic, at_ms, data };
                        if let Ok(text) = serde_json::to_string(&msg) {
                            if socket.send(Message::Text(text.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Ok(ServerMsg::Error { topic, message }) => {
                        if !local.contains(&topic) { continue; }
                        let msg = ServerMsg::Error { topic, message };
                        if let Ok(text) = serde_json::to_string(&msg) {
                            if socket.send(Message::Text(text.into())).await.is_err() {
                                break;
                            }
                        }
                    }
                    Ok(_) => {}
                    Err(broadcast::error::RecvError::Lagged(_)) => {}
                    Err(broadcast::error::RecvError::Closed) => break,
                }
            }
        }
    }

    for t in local.drain() {
        hub.unregister_topic(&t).await;
    }
}

pub async fn ws_upgrade(ws: WebSocketUpgrade, State(state): State<AppState>) -> impl IntoResponse {
    ws.on_upgrade(move |socket| handle_socket(socket, state))
}
