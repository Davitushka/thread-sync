//! Непрерывная генерация событий в формате Serilog (как Python seed) → Vector HTTP `/logs`.

use anyhow::{Context, Result};
use chrono::{SecondsFormat, Utc};
use rand::distributions::{Distribution, WeightedIndex};
use rand::Rng;
use serde_json::{json, Value};
use std::time::{Duration, Instant};
use tokio::time::sleep;
use tracing::{info, warn};
use uuid::Uuid;

const NORMAL_IPS: &[&str] = &[
    "192.168.1.10",
    "192.168.1.11",
    "192.168.1.22",
    "192.168.1.33",
    "192.168.1.44",
];
const ATTACKER_IPS: &[&str] = &[
    "203.0.113.5",
    "203.0.113.12",
    "203.0.113.88",
    "198.51.100.20",
];

#[derive(Debug, Clone)]
struct Settings {
    url: String,
    eps: u32,
    burst_sec: u64,
    sleep_sec: u64,
    threat_ratio: f64,
    batch_size: usize,
    request_timeout: Duration,
}

impl Settings {
    fn from_env() -> Self {
        Self {
            url: std::env::var("SIEM_LOGGEN_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:8080/logs".into()),
            eps: parse_u32_env("SIEM_LOGGEN_EPS", 18).max(1),
            burst_sec: parse_u64_env("SIEM_LOGGEN_BURST_SEC", 90).max(1),
            sleep_sec: parse_u64_env("SIEM_LOGGEN_SLEEP_SEC", 8),
            threat_ratio: parse_f64_env("SIEM_LOGGEN_THREAT_RATIO", 0.05).clamp(0.0, 1.0),
            batch_size: parse_usize_env("SIEM_LOGGEN_BATCH_SIZE", 100).max(1),
            request_timeout: Duration::from_secs(parse_u64_env("SIEM_LOGGEN_HTTP_TIMEOUT_SEC", 10)),
        }
    }
}

fn parse_u32_env(k: &str, d: u32) -> u32 {
    std::env::var(k)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(d)
}

fn parse_u64_env(k: &str, d: u64) -> u64 {
    std::env::var(k)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(d)
}

fn parse_usize_env(k: &str, d: usize) -> usize {
    std::env::var(k)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(d)
}

fn parse_f64_env(k: &str, d: f64) -> f64 {
    std::env::var(k)
        .ok()
        .and_then(|s| s.parse().ok())
        .unwrap_or(d)
}

fn ts() -> String {
    Utc::now().to_rfc3339_opts(SecondsFormat::Millis, true)
}

fn pick_ip<R: Rng>(rng: &mut R, threat: bool) -> &'static str {
    let pool = if threat { ATTACKER_IPS } else { NORMAL_IPS };
    pool[rng.gen_range(0..pool.len())]
}

fn gen_dotnet<R: Rng>(rng: &mut R, threat: bool) -> Value {
    let methods = ["GET", "POST", "PUT", "DELETE", "PATCH"];
    let method = methods[rng.gen_range(0..methods.len())];
    let endpoints = [
        "/api/auth/login",
        "/api/users",
        "/api/products",
        "/api/orders",
        "/api/search",
        "/hubs/notifications",
    ];
    let endpoint = endpoints[rng.gen_range(0..endpoints.len())];
    let (status_code, level) = if threat {
        let codes = [401, 403, 429, 500, 503];
        (codes[rng.gen_range(0..codes.len())], "Warning")
    } else {
        let codes = [200, 200, 201, 204, 301, 304, 400, 404, 422];
        (codes[rng.gen_range(0..codes.len())], "Information")
    };
    let elapsed: f64 = rng.gen_range(5.0..800.0);
    let host = format!("api-{:02}", rng.gen_range(1..=4));
    let msg = format!("HTTP {method} {endpoint} responded {status_code} in {elapsed:.2}ms");
    let user_id: Value = if rng.gen_bool(0.7) {
        json!(Uuid::new_v4().to_string())
    } else {
        Value::Null
    };
    json!({
        "Timestamp": ts(),
        "Level": level,
        "Message": msg,
        "SourceType": "dotnet",
        "Host": host,
        "Properties": {
            "ClientIp": pick_ip(rng, threat),
            "RequestMethod": method,
            "RequestPath": endpoint,
            "StatusCode": status_code,
            "Elapsed": elapsed,
            "UserId": user_id,
            "CorrelationId": Uuid::new_v4().to_string(),
            "MachineName": host,
        }
    })
}

fn gen_postgresql<R: Rng>(rng: &mut R, threat: bool) -> Value {
    let cmds = ["SELECT", "INSERT INTO", "UPDATE", "DELETE FROM"];
    let cmd = cmds[rng.gen_range(0..cmds.len())];
    let tables = ["users", "orders", "products", "sessions", "audit_logs"];
    let table = tables[rng.gen_range(0..tables.len())];
    let duration_ms: f64 = rng.gen_range(1.0..5000.0);
    let (msg, level) = if threat {
        let inj = [
            "' OR '1'='1",
            "UNION SELECT null, username, password FROM users",
        ];
        let i = inj[rng.gen_range(0..inj.len())];
        (
            format!("ERROR: syntax error near '{i}' in query: {cmd} FROM {table}"),
            "Error",
        )
    } else {
        (
            format!("duration: {duration_ms:.3} ms  statement: {cmd} {table} WHERE id=$1"),
            if duration_ms > 1000.0 {
                "Warning"
            } else {
                "Information"
            },
        )
    };
    json!({
        "Timestamp": ts(),
        "Level": level,
        "Message": msg,
        "SourceType": "postgresql",
        "Host": format!("db-{:02}", rng.gen_range(1..=2)),
        "Properties": {
            "duration_ms": duration_ms,
            "rows_affected": rng.gen_range(0..10000_i32),
            "command": cmd,
            "table": table,
        }
    })
}

fn gen_redis<R: Rng>(rng: &mut R, threat: bool) -> Value {
    let ops = ["GET", "SET", "DEL", "EXPIRE", "HGET", "LPUSH"];
    let op = ops[rng.gen_range(0..ops.len())];
    let prefixes = ["session:", "cache:", "rate:", "user:", "lock:"];
    let key = format!(
        "{}{}",
        prefixes[rng.gen_range(0..prefixes.len())],
        &Uuid::new_v4().to_string()[..8]
    );
    let latency_us = rng.gen_range(50..50000_i32);
    let (msg, level) = if threat {
        (
            format!("SLOWLOG: {op} {key} took {latency_us}us — possible enumeration"),
            "Warning",
        )
    } else if latency_us < 1000 {
        (format!("{op} {key} — {latency_us}us"), "Debug")
    } else {
        (format!("{op} {key} — {latency_us}us"), "Warning")
    };
    json!({
        "Timestamp": ts(),
        "Level": level,
        "Message": msg,
        "SourceType": "redis",
        "Host": format!("redis-{:02}", rng.gen_range(1..=2)),
        "Properties": {
            "operation": op,
            "key": key,
            "latency_us": latency_us,
        }
    })
}

fn gen_nginx<R: Rng>(rng: &mut R, threat: bool) -> Value {
    let methods = ["GET", "POST", "GET", "GET", "HEAD"];
    let method = methods[rng.gen_range(0..methods.len())];
    let paths = [
        "/",
        "/index.html",
        "/api/v1/health",
        "/static/app.js",
        "/.env",
        "/admin",
    ];
    let path = paths[rng.gen_range(0..paths.len())];
    let source_ip = pick_ip(rng, threat);
    let (status, level) = if threat {
        let s = [401, 403, 404, 500][rng.gen_range(0..4)];
        (s, "Warning")
    } else {
        let s = [200, 200, 200, 304, 404][rng.gen_range(0..5)];
        (s, if s >= 400 { "Warning" } else { "Information" })
    };
    let bytes_sent = rng.gen_range(200..50000_i32);
    let request_time: f64 = rng.gen_range(0.001..2.5);
    let ts_nginx = ts();
    let msg =
        format!(r#"{source_ip} - - [{ts_nginx}] "{method} {path} HTTP/1.1" {status} {bytes_sent}"#);
    json!({
        "Timestamp": ts(),
        "Level": level,
        "Message": msg,
        "SourceType": "nginx",
        "Host": format!("nginx-{:02}", rng.gen_range(1..=2)),
        "Properties": {
            "remote_addr": source_ip,
            "method": method,
            "path": path,
            "status": status,
            "bytes_sent": bytes_sent,
            "request_time": request_time,
            "user_agent": if threat { "sqlmap/1.7.8" } else { "Mozilla/5.0 (compatible)" },
        }
    })
}

fn next_event<R: Rng>(rng: &mut R, threat: bool) -> Value {
    let weights = [50u32, 20, 15, 15];
    let dist = WeightedIndex::new(weights).expect("weights");
    match dist.sample(rng) {
        0 => gen_dotnet(rng, threat),
        1 => gen_postgresql(rng, threat),
        2 => gen_redis(rng, threat),
        _ => gen_nginx(rng, threat),
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
    let mut body = String::with_capacity(batch.len() * 256);
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

async fn run_burst(client: &reqwest::Client, cfg: &Settings) -> Result<(usize, usize)> {
    let mut rng = rand::thread_rng();
    let interval = Duration::from_secs_f64(1.0 / cfg.eps as f64);
    let deadline = Instant::now() + Duration::from_secs(cfg.burst_sec);
    let mut sent = 0usize;
    let mut errors = 0usize;
    let mut batch: Vec<Value> = Vec::with_capacity(cfg.batch_size);

    while Instant::now() < deadline {
        let threat = rng.gen_bool(cfg.threat_ratio);
        batch.push(next_event(&mut rng, threat));

        if batch.len() >= cfg.batch_size {
            match post_ndjson(client, &cfg.url, &batch, cfg.request_timeout).await {
                Ok(n) => {
                    sent += n;
                }
                Err(e) => {
                    warn!(error = %e, "batch failed");
                    errors += batch.len();
                }
            }
            batch.clear();
        }

        sleep(interval).await;
    }

    if !batch.is_empty() {
        match post_ndjson(client, &cfg.url, &batch, cfg.request_timeout).await {
            Ok(n) => sent += n,
            Err(e) => {
                warn!(error = %e, "final batch failed");
                errors += batch.len();
            }
        }
    }

    Ok((sent, errors))
}

#[tokio::main]
async fn main() -> Result<()> {
    tracing_subscriber::fmt()
        .with_env_filter(
            tracing_subscriber::EnvFilter::try_from_default_env().unwrap_or_else(|_| {
                tracing_subscriber::EnvFilter::new("siem_log_generator=info,info")
            }),
        )
        .init();

    let cfg = Settings::from_env();
    info!(
        url = %cfg.url,
        eps = cfg.eps,
        burst_sec = cfg.burst_sec,
        sleep_sec = cfg.sleep_sec,
        "siem-log-generator started"
    );

    let client = reqwest::Client::builder()
        .use_rustls_tls()
        .build()
        .context("build HTTP client")?;

    loop {
        let start = Instant::now();
        match run_burst(&client, &cfg).await {
            Ok((s, err)) => {
                info!(
                    sent = s,
                    errors = err,
                    elapsed_sec = start.elapsed().as_secs_f64(),
                    "burst finished"
                );
            }
            Err(e) => {
                warn!(error = %e, "burst error");
            }
        }

        tokio::select! {
            _ = sleep(Duration::from_secs(cfg.sleep_sec)) => {}
            _ = tokio::signal::ctrl_c() => {
                info!("shutdown");
                break;
            }
        }
    }

    Ok(())
}
