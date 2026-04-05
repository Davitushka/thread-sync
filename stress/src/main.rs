use anyhow::{Context, Result};
use chrono::{Duration as ChronoDuration, SecondsFormat, Utc};
use rand::Rng;
use serde_json::{json, Value};
use std::collections::VecDeque;
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{info, warn};
use uuid::Uuid;

const NORMAL_IPS: &[&str] = &[
    "192.168.1.10",
    "192.168.1.11",
    "192.168.1.12",
    "192.168.1.13",
    "192.168.1.14",
    "192.168.1.20",
    "192.168.1.21",
    "192.168.1.22",
    "192.168.1.23",
    "192.168.1.24",
    "192.168.1.30",
    "192.168.1.31",
    "192.168.1.32",
    "192.168.1.33",
    "192.168.1.34",
    "192.168.1.40",
    "192.168.1.41",
    "192.168.1.42",
    "192.168.1.43",
    "192.168.1.44",
    "192.168.1.45",
    "192.168.1.46",
    "192.168.1.47",
    "192.168.1.48",
    "192.168.1.49",
    "192.168.1.50",
];

const ATTACKER_IPS: &[&str] = &[
    "203.0.113.5",
    "203.0.113.12",
    "198.51.100.20",
    "203.0.113.88",
];

const NORMAL_API_PATHS: &[&str] = &[
    "/api/users",
    "/api/products",
    "/api/orders",
    "/api/search",
    "/api/profile",
    "/api/cart",
];

const AUTH_PATHS: &[&str] = &[
    "/api/auth/login",
    "/api/auth/token",
    "/hubs/notifications",
];

const ADMIN_PATHS: &[&str] = &[
    "/admin",
    "/admin/users",
    "/api/admin/config",
    "/api/admin/users",
    "/api/permissions",
];

const RATE_LIMIT_PATHS: &[&str] = &["/api/search", "/api/products"];

const PRODUCT_HOSTS: &[&str] = &["api-01", "api-02", "api-03", "api-04"];
const DB_HOSTS: &[&str] = &["db-01", "db-02"];
const REDIS_HOSTS: &[&str] = &["redis-01", "redis-02"];
const NGINX_HOSTS: &[&str] = &["nginx-01", "nginx-02"];

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
enum Mode {
    Normal,
    BruteForce,
    SqlInjection,
    PrivilegeEscalation,
    RateLimit,
    HeavyQueries,
    All,
}

impl Mode {
    fn parse(s: &str) -> Self {
        match s.trim().to_ascii_lowercase().as_str() {
            "normal" => Self::Normal,
            "brute-force" => Self::BruteForce,
            "sql-injection" => Self::SqlInjection,
            "privilege-escalation" => Self::PrivilegeEscalation,
            "rate-limit" => Self::RateLimit,
            "heavy-queries" => Self::HeavyQueries,
            _ => Self::All,
        }
    }
}

#[derive(Debug, Clone)]
struct Settings {
    mode: Mode,
    url: String,
    duration_sec: u64,
    normal_eps: u32,
    attack_eps: u32,
    burst_interval_sec: u64,
    batch_size: usize,
    request_timeout: Duration,
}

impl Settings {
    fn from_env() -> Self {
        Self {
            mode: Mode::parse(
                &std::env::var("SIEM_STRESS_MODE").unwrap_or_else(|_| "all".into()),
            ),
            url: std::env::var("SIEM_STRESS_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8080/logs".into()),
            duration_sec: parse_u64_env("SIEM_STRESS_DURATION_SEC", 300),
            normal_eps: parse_u32_env("SIEM_STRESS_NORMAL_EPS", 50).max(1),
            attack_eps: parse_u32_env("SIEM_STRESS_ATTACK_EPS", 10).max(1),
            burst_interval_sec: parse_u64_env("SIEM_STRESS_BURST_INTERVAL_SEC", 60).max(1),
            batch_size: parse_usize_env("SIEM_STRESS_BATCH_SIZE", 100).max(1),
            request_timeout: Duration::from_secs(parse_u64_env("SIEM_STRESS_HTTP_TIMEOUT_SEC", 10)),
        }
    }
}

#[derive(Debug)]
struct RuntimeState {
    brute_force_attempts: VecDeque<Instant>,
    rate_limit_attempts: VecDeque<Instant>,
    privilege_attempts: VecDeque<Instant>,
}

impl RuntimeState {
    fn new() -> Self {
        Self {
            brute_force_attempts: VecDeque::new(),
            rate_limit_attempts: VecDeque::new(),
            privilege_attempts: VecDeque::new(),
        }
    }
}

fn parse_u32_env(k: &str, d: u32) -> u32 {
    std::env::var(k).ok().and_then(|s| s.parse().ok()).unwrap_or(d)
}

fn parse_u64_env(k: &str, d: u64) -> u64 {
    std::env::var(k).ok().and_then(|s| s.parse().ok()).unwrap_or(d)
}

fn parse_usize_env(k: &str, d: usize) -> usize {
    std::env::var(k).ok().and_then(|s| s.parse().ok()).unwrap_or(d)
}

fn ts_now() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn ts_past(max_days: i64) -> String {
    let mut rng = rand::thread_rng();
    let mins = rng.gen_range(0..(max_days * 24 * 60).max(1));
    (Utc::now() - ChronoDuration::minutes(mins))
        .to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn pick<'a, R: Rng>(rng: &mut R, items: &'a [&'a str]) -> &'a str {
    items[rng.gen_range(0..items.len())]
}

fn random_user<R: Rng>(rng: &mut R) -> String {
    format!("user_{:03}", rng.gen_range(1..=250))
}

fn normal_ip<R: Rng>(rng: &mut R) -> &'static str {
    pick(rng, NORMAL_IPS)
}

fn attacker_ip<R: Rng>(rng: &mut R) -> &'static str {
    pick(rng, ATTACKER_IPS)
}

fn heavy_elapsed_ms<R: Rng>(rng: &mut R) -> f64 {
    let roll: f64 = rng.gen_range(0.0..1.0);
    if roll < 0.05 {
        rng.gen_range(5000.0..15000.0)
    } else if roll < 0.15 {
        rng.gen_range(1000.0..5000.0)
    } else {
        rng.gen_range(10.0..950.0)
    }
}

fn level_for_status(status_code: i32) -> &'static str {
    match status_code {
        500..=599 => "Error",
        400..=499 => "Warning",
        _ => "Information",
    }
}

fn build_dotnet_event(
    timestamp: String,
    level: &str,
    message: String,
    host: &str,
    ip: &str,
    method: &str,
    path: &str,
    status_code: i32,
    elapsed: f64,
    user_id: Value,
    extra_properties: Value,
) -> Value {
    let mut props = serde_json::Map::new();
    props.insert("ClientIp".into(), json!(ip));
    props.insert("RequestMethod".into(), json!(method));
    props.insert("RequestPath".into(), json!(path));
    props.insert("StatusCode".into(), json!(status_code));
    props.insert("Elapsed".into(), json!(elapsed));
    props.insert("UserId".into(), user_id);
    props.insert("CorrelationId".into(), json!(Uuid::new_v4().to_string()));
    props.insert("MachineName".into(), json!(host));
    if let Some(obj) = extra_properties.as_object() {
        for (k, v) in obj {
            props.insert(k.clone(), v.clone());
        }
    }

    json!({
        "Timestamp": timestamp,
        "Level": level,
        "Message": message,
        "SourceType": "dotnet",
        "Host": host,
        "Properties": props
    })
}

fn build_postgres_event(
    timestamp: String,
    level: &str,
    message: String,
    host: &str,
    duration_ms: f64,
    command: &str,
    table: &str,
    ip: Option<&str>,
) -> Value {
    json!({
        "Timestamp": timestamp,
        "Level": level,
        "Message": message,
        "SourceType": "postgresql",
        "Host": host,
        "Properties": {
            "duration_ms": duration_ms,
            "command": command,
            "table": table,
            "ClientIp": ip,
            "rows_affected": rand::thread_rng().gen_range(0..5000_i32)
        }
    })
}

fn build_redis_event(
    timestamp: String,
    level: &str,
    message: String,
    host: &str,
    operation: &str,
    key: &str,
    latency_us: i64,
    ip: Option<&str>,
) -> Value {
    json!({
        "Timestamp": timestamp,
        "Level": level,
        "Message": message,
        "SourceType": "redis",
        "Host": host,
        "Properties": {
            "operation": operation,
            "key": key,
            "latency_us": latency_us,
            "ClientIp": ip
        }
    })
}

fn build_nginx_event(
    timestamp: String,
    level: &str,
    host: &str,
    ip: &str,
    method: &str,
    path: &str,
    status: i32,
    request_time_sec: f64,
    bytes_sent: i32,
    user_agent: &str,
) -> Value {
    let msg = format!(
        r#"{ip} - - [{timestamp}] "{method} {path} HTTP/1.1" {status} {bytes_sent}"#
    );
    json!({
        "Timestamp": ts_now(),
        "Level": level,
        "Message": msg,
        "SourceType": "nginx",
        "Host": host,
        "Properties": {
            "remote_addr": ip,
            "method": method,
            "path": path,
            "status": status,
            "request_time": request_time_sec,
            "bytes_sent": bytes_sent,
            "UserAgent": user_agent
        }
    })
}

fn gen_normal<R: Rng>(rng: &mut R) -> Vec<Value> {
    let source = rng.gen_range(0..4);
    match source {
        0 => {
            let method = if rng.gen_bool(0.65) { "GET" } else { "POST" };
            let path = pick(rng, NORMAL_API_PATHS);
            let status_code = if method == "POST" {
                [200, 201, 204][rng.gen_range(0..3)]
            } else {
                [200, 200, 204][rng.gen_range(0..3)]
            };
            let elapsed = heavy_elapsed_ms(rng);
            let host = pick(rng, PRODUCT_HOSTS);
            let ip = normal_ip(rng);
            let user = if rng.gen_bool(0.8) {
                json!(random_user(rng))
            } else {
                Value::Null
            };
            vec![build_dotnet_event(
                ts_now(),
                level_for_status(status_code),
                format!("HTTP {method} {path} responded {status_code} in {elapsed:.2}ms"),
                host,
                ip,
                method,
                path,
                status_code,
                elapsed,
                user,
                json!({
                    "UserAgent": "Mozilla/5.0",
                    "UserRole": "user"
                }),
            )]
        }
        1 => {
            let cmd = ["SELECT", "INSERT", "UPDATE"][rng.gen_range(0..3)];
            let table = ["users", "products", "orders"][rng.gen_range(0..3)];
            let duration_ms = heavy_elapsed_ms(rng);
            vec![build_postgres_event(
                ts_now(),
                if duration_ms > 5000.0 {
                    "Warning"
                } else {
                    "Information"
                },
                format!("duration: {duration_ms:.3} ms statement: {cmd} * FROM {table} WHERE id=$1"),
                pick(rng, DB_HOSTS),
                duration_ms,
                cmd,
                table,
                Some(normal_ip(rng)),
            )]
        }
        2 => {
            let op = ["GET", "SET", "DEL"][rng.gen_range(0..3)];
            let key = format!("cache:{}:{}", pick(rng, &["users", "orders", "products"]), rng.gen_range(1000..9999));
            let latency_us = if rng.gen_bool(0.1) {
                rng.gen_range(100_000..300_000_i64)
            } else {
                rng.gen_range(120..30_000_i64)
            };
            vec![build_redis_event(
                ts_now(),
                if latency_us > 100_000 { "Warning" } else { "Information" },
                format!("{op} {key} completed in {latency_us}us"),
                pick(rng, REDIS_HOSTS),
                op,
                &key,
                latency_us,
                Some(normal_ip(rng)),
            )]
        }
        _ => {
            let method = ["GET", "GET", "POST"][rng.gen_range(0..3)];
            let path = pick(rng, NORMAL_API_PATHS);
            let status = [200, 200, 201, 204][rng.gen_range(0..4)];
            vec![build_nginx_event(
                ts_now(),
                "Information",
                pick(rng, NGINX_HOSTS),
                normal_ip(rng),
                method,
                path,
                status,
                rng.gen_range(0.005..2.2),
                rng.gen_range(200..25_000),
                "Mozilla/5.0",
            )]
        }
    }
}

fn gen_brute_force<R: Rng>(rng: &mut R, state: &mut RuntimeState) -> Vec<Value> {
    let ip = attacker_ip(rng);
    let path = pick(rng, AUTH_PATHS);
    let status_code = [401, 403][rng.gen_range(0..2)];
    let elapsed = rng.gen_range(15.0..120.0);
    state.brute_force_attempts.push_back(Instant::now());
    while let Some(front) = state.brute_force_attempts.front() {
        if front.elapsed() > Duration::from_secs(120) {
            state.brute_force_attempts.pop_front();
        } else {
            break;
        }
    }

    vec![
        build_dotnet_event(
            ts_now(),
            if status_code == 403 { "Error" } else { "Warning" },
            format!("Authentication failed for user admin from {ip}"),
            pick(rng, PRODUCT_HOSTS),
            ip,
            "POST",
            path,
            status_code,
            elapsed,
            json!("admin"),
            json!({
                "UserAgent": "Hydra/9.5",
                "UserRole": "anonymous"
            }),
        ),
        build_nginx_event(
            ts_now(),
            "Warning",
            pick(rng, NGINX_HOSTS),
            ip,
            "POST",
            path,
            status_code,
            rng.gen_range(0.01..0.35),
            rng.gen_range(200..900),
            "Hydra/9.5",
        ),
    ]
}

fn gen_sql_injection<R: Rng>(rng: &mut R) -> Vec<Value> {
    let ip = attacker_ip(rng);
    let payloads = [
        "/api/users?id=1' OR '1'='1",
        "/api/search?q=UNION SELECT username,password FROM users",
        "/api/orders?id=1; DROP TABLE orders",
        "/api/products?filter=' OR 1=1--",
    ];
    let path = pick(rng, &payloads);
    let status_code = [400, 500][rng.gen_range(0..2)];
    let elapsed = rng.gen_range(120.0..1800.0);
    let message = [
        "UNION SELECT username,password FROM users",
        "' OR 1=1--",
        "DROP TABLE orders",
        "information_schema.tables",
    ][rng.gen_range(0..4)];

    vec![
        build_dotnet_event(
            ts_now(),
            "Error",
            format!("HTTP GET {path} responded {status_code} in {elapsed:.2}ms; detected payload {message}"),
            pick(rng, PRODUCT_HOSTS),
            ip,
            "GET",
            path,
            status_code,
            elapsed,
            json!(random_user(rng)),
            json!({
                "UserAgent": "sqlmap/1.7.8",
                "UserRole": "user"
            }),
        ),
        build_postgres_event(
            ts_now(),
            "Error",
            format!("ERROR: syntax error near '{message}' in statement: SELECT * FROM users WHERE id = '{message}'"),
            pick(rng, DB_HOSTS),
            rng.gen_range(100.0..9000.0),
            "SELECT",
            "users",
            Some(ip),
        ),
    ]
}

fn gen_privilege_escalation<R: Rng>(rng: &mut R, state: &mut RuntimeState) -> Vec<Value> {
    let ip = normal_ip(rng);
    let path = pick(rng, ADMIN_PATHS);
    let user = random_user(rng);
    let method = ["GET", "POST", "PATCH"][rng.gen_range(0..3)];
    let status_code = 403;
    let elapsed = rng.gen_range(20.0..250.0);

    state.privilege_attempts.push_back(Instant::now());
    while let Some(front) = state.privilege_attempts.front() {
        if front.elapsed() > Duration::from_secs(300) {
            state.privilege_attempts.pop_front();
        } else {
            break;
        }
    }

    vec![
        build_dotnet_event(
            ts_now(),
            "Error",
            format!("Unauthorized access attempt to {path} by user {user}"),
            pick(rng, PRODUCT_HOSTS),
            ip,
            method,
            path,
            status_code,
            elapsed,
            json!(user),
            json!({
                "UserAgent": "Mozilla/5.0",
                "UserRole": "user"
            }),
        ),
        build_nginx_event(
            ts_now(),
            "Warning",
            pick(rng, NGINX_HOSTS),
            ip,
            method,
            path,
            status_code,
            rng.gen_range(0.02..0.45),
            rng.gen_range(150..850),
            "Mozilla/5.0",
        ),
    ]
}

fn gen_rate_limit<R: Rng>(rng: &mut R, state: &mut RuntimeState) -> Vec<Value> {
    let ip = "203.0.113.88";
    let path = pick(rng, RATE_LIMIT_PATHS);
    let elapsed = rng.gen_range(3.0..30.0);
    state.rate_limit_attempts.push_back(Instant::now());
    while let Some(front) = state.rate_limit_attempts.front() {
        if front.elapsed() > Duration::from_secs(60) {
            state.rate_limit_attempts.pop_front();
        } else {
            break;
        }
    }
    let count = state.rate_limit_attempts.len().max(500);

    vec![
        build_dotnet_event(
            ts_now(),
            "Warning",
            format!("Rate limit exceeded: {count} requests in 60s from {ip}"),
            pick(rng, PRODUCT_HOSTS),
            ip,
            "GET",
            path,
            429,
            elapsed,
            json!(Value::Null),
            json!({
                "UserAgent": "attack-bot/1.0"
            }),
        ),
        build_nginx_event(
            ts_now(),
            "Warning",
            pick(rng, NGINX_HOSTS),
            ip,
            "GET",
            path,
            429,
            rng.gen_range(0.001..0.05),
            rng.gen_range(80..400),
            "attack-bot/1.0",
        ),
    ]
}

fn gen_heavy_queries<R: Rng>(rng: &mut R) -> Vec<Value> {
    let ip = if rng.gen_bool(0.7) { normal_ip(rng) } else { attacker_ip(rng) };
    let duration_ms = rng.gen_range(10_000.0..20_000.0);
    let redis_latency = rng.gen_range(100_000..600_000_i64);
    let path = pick(rng, NORMAL_API_PATHS);

    vec![
        build_postgres_event(
            ts_now(),
            "Warning",
            format!(
                "duration: {duration_ms:.3} ms statement: SELECT * FROM orders JOIN users JOIN products WHERE orders.user_id = users.id AND products.id = orders.product_id"
            ),
            pick(rng, DB_HOSTS),
            duration_ms,
            "SELECT",
            "orders",
            Some(ip),
        ),
        build_redis_event(
            ts_now(),
            "Warning",
            format!(
                "SLOWLOG: {} {} took {}us",
                "GET",
                "cache:analytics:dashboard",
                redis_latency
            ),
            pick(rng, REDIS_HOSTS),
            "GET",
            "cache:analytics:dashboard",
            redis_latency,
            Some(ip),
        ),
        build_dotnet_event(
            ts_now(),
            if duration_ms > 12000.0 { "Error" } else { "Warning" },
            format!("HTTP GET {path} responded 200 in {duration_ms:.2}ms"),
            pick(rng, PRODUCT_HOSTS),
            ip,
            "GET",
            path,
            200,
            duration_ms,
            json!(random_user(rng)),
            json!({
                "UserAgent": "Mozilla/5.0",
                "UserRole": "user"
            }),
        ),
    ]
}

fn gen_mode_events<R: Rng>(rng: &mut R, mode: Mode, state: &mut RuntimeState) -> Vec<Value> {
    match mode {
        Mode::Normal => gen_normal(rng),
        Mode::BruteForce => gen_brute_force(rng, state),
        Mode::SqlInjection => gen_sql_injection(rng),
        Mode::PrivilegeEscalation => gen_privilege_escalation(rng, state),
        Mode::RateLimit => gen_rate_limit(rng, state),
        Mode::HeavyQueries => gen_heavy_queries(rng),
        Mode::All => {
            let roll: u32 = rng.gen_range(0..65);
            if roll < 50 {
                gen_normal(rng)
            } else if roll < 55 {
                gen_brute_force(rng, state)
            } else if roll < 58 {
                gen_sql_injection(rng)
            } else if roll < 60 {
                gen_privilege_escalation(rng, state)
            } else if roll < 65 {
                gen_heavy_queries(rng)
            } else {
                gen_rate_limit(rng, state)
            }
        }
    }
}

async fn post_ndjson(
    client: &reqwest::Client,
    url: &str,
    batch: &[Value],
    timeout: Duration,
) -> Result<usize> {
    if batch.is_empty() {
        return Ok(0);
    }
    let mut body = String::with_capacity(batch.len() * 512);
    for v in batch {
        body.push_str(&serde_json::to_string(v)?);
        body.push('\n');
    }

    let resp = client
        .post(url)
        .header("Content-Type", "application/x-ndjson")
        .timeout(timeout)
        .body(body)
        .send()
        .await
        .context("HTTP post")?;

    if !resp.status().is_success() {
        let status = resp.status();
        let text = resp.text().await.unwrap_or_default();
        anyhow::bail!("Vector returned {status}: {text}");
    }

    Ok(batch.len())
}

async fn run_once(client: &reqwest::Client, cfg: &Settings) -> Result<(usize, usize)> {
    let mut rng = rand::thread_rng();
    let mut state = RuntimeState::new();
    let mut sent = 0usize;
    let mut failed = 0usize;
    let mut batch: Vec<Value> = Vec::with_capacity(cfg.batch_size);
    let start = Instant::now();
    let deadline = if cfg.duration_sec == 0 {
        None
    } else {
        Some(start + Duration::from_secs(cfg.duration_sec))
    };

    let target_eps = match cfg.mode {
        Mode::Normal => cfg.normal_eps,
        Mode::All => cfg.normal_eps + cfg.attack_eps + 5,
        _ => cfg.attack_eps,
    }
    .max(1);
    let interval = Duration::from_secs_f64(1.0 / target_eps as f64);
    let mut next_rate_burst = Instant::now();

    loop {
        if let Some(deadline) = deadline {
            if Instant::now() >= deadline {
                break;
            }
        }

        let mut events = if cfg.mode == Mode::All {
            let mut items = gen_mode_events(&mut rng, cfg.mode, &mut state);
            if Instant::now() >= next_rate_burst {
                next_rate_burst = Instant::now() + Duration::from_secs(cfg.burst_interval_sec);
                for _ in 0..600 {
                    items.extend(gen_rate_limit(&mut rng, &mut state));
                }
            }
            items
        } else {
            gen_mode_events(&mut rng, cfg.mode, &mut state)
        };

        batch.append(&mut events);

        if batch.len() >= cfg.batch_size {
            match post_ndjson(client, &cfg.url, &batch, cfg.request_timeout).await {
                Ok(n) => sent += n,
                Err(e) => {
                    warn!(error = %e, failed_events = batch.len(), "stress batch failed");
                    failed += batch.len();
                }
            }
            batch.clear();
        }

        tokio::select! {
            _ = sleep(interval) => {}
            _ = tokio::signal::ctrl_c() => {
                info!("shutdown requested");
                break;
            }
        }
    }

    if !batch.is_empty() {
        match post_ndjson(client, &cfg.url, &batch, cfg.request_timeout).await {
            Ok(n) => sent += n,
            Err(e) => {
                warn!(error = %e, failed_events = batch.len(), "final stress batch failed");
                failed += batch.len();
            }
        }
    }

    Ok((sent, failed))
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env()
                .unwrap_or_else(|_| tracing_subscriber::EnvFilter::new("siem_stress=info,info")),
        )
        .init();

    let cfg = Settings::from_env();
    info!(
        mode = ?cfg.mode,
        url = %cfg.url,
        duration_sec = cfg.duration_sec,
        normal_eps = cfg.normal_eps,
        attack_eps = cfg.attack_eps,
        burst_interval_sec = cfg.burst_interval_sec,
        batch_size = cfg.batch_size,
        "siem-stress started"
    );

    let client = reqwest::Client::builder()
        .use_rustls_tls()
        .build()
        .context("build HTTP client")?;

    let run_started = Instant::now();
    let (sent, failed) = run_once(&client, &cfg).await?;
    info!(
        sent,
        failed,
        elapsed_sec = run_started.elapsed().as_secs_f64(),
        "siem-stress finished"
    );
    Ok(())
}
