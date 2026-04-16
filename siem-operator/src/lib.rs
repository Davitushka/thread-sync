//! SIEM-Lite Operator: модули + egui + опционально WebView к порталу (один exe).
pub mod app;
pub mod models;
pub mod theme;
pub mod ui;

use std::path::{Path, PathBuf};
use std::process::{Child, Command, Stdio};
use std::time::{Duration, Instant};

use eframe::egui;
use reqwest::Url;
use tao::{
    dpi::LogicalSize,
    event::{ElementState, Event, WindowEvent},
    event_loop::{ControlFlow, EventLoopBuilder, EventLoopProxy},
    keyboard::{Key, KeyCode, ModifiersState},
    window::WindowBuilder,
};
use wry::WebViewBuilder;

const DEFAULT_PORTAL_URL: &str = "http://127.0.0.1:8091/";

fn report_if_err<T, E: std::fmt::Display>(result: Result<T, E>, action: &str) {
    if let Err(err) = result {
        eprintln!("siem-operator: {action}: {err}");
    }
}

fn normalize_portal_url(raw: &str) -> String {
    let trimmed = raw.trim();
    if trimmed.is_empty() {
        return DEFAULT_PORTAL_URL.to_string();
    }
    let Ok(mut url) = Url::parse(trimmed) else {
        return trimmed.to_string();
    };
    if matches!(
        url.host_str(),
        Some("localhost") | Some("::1") | Some("[::1]")
    ) {
        let _ = url.set_host(Some("127.0.0.1"));
    }
    if url.path().is_empty() {
        url.set_path("/");
    }
    url.to_string()
}

/// URL портала для окна WebView (`--web`).
pub fn portal_url() -> String {
    let raw = std::env::var("SIEM_OPERATOR_PORTAL_URL")
        .unwrap_or_else(|_| DEFAULT_PORTAL_URL.to_string());
    normalize_portal_url(&raw)
}

/// Нативный клиент на egui (все вкладки и логика).
pub fn run_egui_operator() -> eframe::Result<()> {
    let options = eframe::NativeOptions {
        viewport: egui::ViewportBuilder::default()
            .with_title("SIEM-Lite Operator")
            .with_inner_size([1180.0, 760.0])
            .with_min_inner_size([900.0, 560.0]),
        ..Default::default()
    };
    eframe::run_native(
        "SIEM-Lite Operator",
        options,
        Box::new(|cc| {
            theme::setup_theme(&cc.egui_ctx);
            Ok(Box::new(app::OperatorApp::default()) as Box<dyn eframe::App>)
        }),
    )
}

fn portal_health_urls(raw: &str) -> Vec<String> {
    let Ok(url) = Url::parse(raw) else {
        return Vec::new();
    };
    let mut out = Vec::new();
    let mut primary = url.clone();
    primary.set_path("/health");
    primary.set_query(None);
    primary.set_fragment(None);
    out.push(primary.to_string());
    if matches!(
        url.host_str(),
        Some("localhost") | Some("::1") | Some("[::1]")
    ) {
        let mut fallback = url;
        if fallback.set_host(Some("127.0.0.1")).is_ok() {
            fallback.set_path("/health");
            fallback.set_query(None);
            fallback.set_fragment(None);
            let candidate = fallback.to_string();
            if !out.contains(&candidate) {
                out.push(candidate);
            }
        }
    }
    out
}

fn portal_is_local(raw: &str) -> bool {
    let Ok(url) = Url::parse(raw) else {
        return false;
    };
    matches!(
        url.host_str(),
        Some("127.0.0.1") | Some("localhost") | Some("::1") | Some("[::1]")
    )
}

fn portal_autostart_enabled() -> bool {
    std::env::var("SIEM_OPERATOR_AUTOSTART_PORTAL")
        .map(|v| {
            !matches!(
                v.trim().to_ascii_lowercase().as_str(),
                "0" | "false" | "no" | "off"
            )
        })
        .unwrap_or(true)
}

fn portal_ready(raw: &str, timeout: Duration) -> bool {
    let health_urls = portal_health_urls(raw);
    if health_urls.is_empty() {
        return false;
    }
    let Ok(client) = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
    else {
        return false;
    };
    health_urls.into_iter().any(|health_url| {
        client
            .get(health_url)
            .send()
            .map(|resp| resp.status().is_success())
            .unwrap_or(false)
    })
}

fn wait_for_portal(raw: &str, timeout: Duration) -> bool {
    let deadline = Instant::now() + timeout;
    while Instant::now() < deadline {
        if portal_ready(raw, Duration::from_secs(2)) {
            return true;
        }
        std::thread::sleep(Duration::from_millis(400));
    }
    false
}

fn repo_root_candidates() -> Vec<PathBuf> {
    let mut out = Vec::new();

    if let Ok(current_dir) = std::env::current_dir() {
        for ancestor in current_dir.ancestors() {
            let candidate = ancestor.to_path_buf();
            if !out.contains(&candidate) {
                out.push(candidate);
            }
        }
    }

    if let Ok(current_exe) = std::env::current_exe() {
        for ancestor in current_exe.ancestors() {
            let candidate = ancestor.to_path_buf();
            if !out.contains(&candidate) {
                out.push(candidate);
            }
        }
    }

    out
}

fn locate_repo_root() -> Option<PathBuf> {
    repo_root_candidates()
        .into_iter()
        .find(|base| base.join("siem-portal").join("Cargo.toml").is_file())
}

fn portal_bind_from_url(raw: &str) -> Option<String> {
    let url = Url::parse(raw).ok()?;
    let port = url.port_or_known_default()?;
    Some(format!("127.0.0.1:{port}"))
}

fn portal_binary_candidates(repo_root: &Path) -> Vec<PathBuf> {
    let exe = if cfg!(windows) {
        "siem-portal.exe"
    } else {
        "siem-portal"
    };

    vec![
        repo_root
            .join("siem-portal")
            .join("target")
            .join("release")
            .join(exe),
        repo_root.join("target").join("release").join(exe),
    ]
}

#[cfg(windows)]
fn configure_background_command(cmd: &mut Command) {
    use std::os::windows::process::CommandExt;

    const CREATE_NO_WINDOW: u32 = 0x0800_0000;
    cmd.creation_flags(CREATE_NO_WINDOW);
}

#[cfg(not(windows))]
fn configure_background_command(_cmd: &mut Command) {}

fn set_default_portal_env(cmd: &mut Command, key: &str, value: &str) {
    if std::env::var_os(key).is_none() {
        cmd.env(key, value);
    }
}

fn apply_local_portal_defaults(cmd: &mut Command, portal_url: &str) {
    if let Some(bind) = portal_bind_from_url(portal_url) {
        cmd.env("SIEM_PORTAL_ADDR", bind);
    }

    set_default_portal_env(cmd, "SIEM_PORTAL_CASEMGMT_URL", "http://127.0.0.1:8088");
    set_default_portal_env(cmd, "SIEM_PORTAL_PROMETHEUS_URL", "http://127.0.0.1:9090");
    set_default_portal_env(cmd, "SIEM_PORTAL_ALERTMANAGER_URL", "http://127.0.0.1:9093");
    set_default_portal_env(cmd, "SIEM_PORTAL_CORRELATOR_URL", "http://127.0.0.1:9111");
    set_default_portal_env(cmd, "SIEM_PORTAL_GRAFANA_URL", "http://127.0.0.1:3000");
    set_default_portal_env(cmd, "SIEM_PORTAL_CLICKHOUSE_URL", "http://127.0.0.1:8123");
    set_default_portal_env(cmd, "SIEM_PORTAL_PUBLIC_GRAFANA", "http://127.0.0.1:3000");
    set_default_portal_env(
        cmd,
        "SIEM_PORTAL_PUBLIC_PROMETHEUS",
        "http://127.0.0.1:9090",
    );
    set_default_portal_env(
        cmd,
        "SIEM_PORTAL_PUBLIC_ALERTMANAGER",
        "http://127.0.0.1:9093",
    );
    set_default_portal_env(cmd, "SIEM_PORTAL_PUBLIC_CASEMGMT", "http://127.0.0.1:8088");
    set_default_portal_env(
        cmd,
        "SIEM_PORTAL_PUBLIC_GRAFANA_OVERVIEW",
        "http://127.0.0.1:3000/d/siem-overview",
    );
}

fn spawn_portal_process(repo_root: &Path, portal_url: &str) -> std::io::Result<Child> {
    if let Ok(explicit_bin) = std::env::var("SIEM_OPERATOR_PORTAL_BIN") {
        let explicit_bin = explicit_bin.trim();
        if !explicit_bin.is_empty() {
            let mut cmd = Command::new(explicit_bin);
            apply_local_portal_defaults(&mut cmd, portal_url);
            cmd.stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            configure_background_command(&mut cmd);
            return cmd.spawn();
        }
    }

    for bin in portal_binary_candidates(repo_root) {
        if bin.is_file() {
            let mut cmd = Command::new(bin);
            apply_local_portal_defaults(&mut cmd, portal_url);
            cmd.stdin(Stdio::null())
                .stdout(Stdio::null())
                .stderr(Stdio::null());
            configure_background_command(&mut cmd);
            return cmd.spawn();
        }
    }

    let manifest = repo_root.join("siem-portal").join("Cargo.toml");
    if !manifest.is_file() {
        return Err(std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "siem-portal/Cargo.toml not found рядом с siem-operator",
        ));
    }

    let mut cmd = Command::new("cargo");
    cmd.current_dir(repo_root)
        .arg("run")
        .arg("--release")
        .arg("--manifest-path")
        .arg(manifest);
    apply_local_portal_defaults(&mut cmd, portal_url);
    cmd.stdin(Stdio::null())
        .stdout(Stdio::null())
        .stderr(Stdio::null());
    configure_background_command(&mut cmd);
    cmd.spawn()
}

fn ensure_portal_available(raw: &str) -> std::io::Result<Option<Child>> {
    if portal_ready(raw, Duration::from_secs(2)) {
        return Ok(None);
    }

    if !portal_is_local(raw) {
        return Err(std::io::Error::other(format!(
            "portal is unavailable at {raw}. Check the remote URL, VPN/proxy/firewall, or start the suite in a browser first"
        )));
    }

    if !portal_autostart_enabled() {
        return Err(std::io::Error::other(format!(
            "portal is unavailable at {raw} and auto-start is disabled. Start `siem-portal` manually or enable SIEM_OPERATOR_AUTOSTART_PORTAL"
        )));
    }

    let repo_root = locate_repo_root().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Не удалось найти репозиторий рядом с siem-operator для автозапуска siem-portal",
        )
    })?;
    let mut child = spawn_portal_process(&repo_root, raw)?;

    if wait_for_portal(raw, Duration::from_secs(30)) {
        Ok(Some(child))
    } else {
        let _ = child.kill();
        Err(std::io::Error::other(
            "siem-operator запустил siem-portal, но портал не поднялся за 30 секунд",
        ))
    }
}

enum UserEvent {
    PortalBootstrapFinished(Result<Option<Child>, String>),
    RetryPortalBootstrap,
    OpenPortalInBrowser,
    UpdateTitle(String),
}

fn operator_window_title(document_title: Option<&str>) -> String {
    let title = document_title.unwrap_or("Unified Suite").trim();
    if title.is_empty() {
        "SIEM-Lite Operator".to_string()
    } else if title.starts_with("SIEM-Lite Operator") {
        title.to_string()
    } else {
        format!("SIEM-Lite Operator · {title}")
    }
}

fn escape_html(value: &str) -> String {
    value
        .replace('&', "&amp;")
        .replace('<', "&lt;")
        .replace('>', "&gt;")
        .replace('"', "&quot;")
        .replace('\'', "&#39;")
}

fn loading_screen_html(url: &str, status: &str) -> String {
    let url = escape_html(url);
    let status = escape_html(status);
    format!(
        r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width,initial-scale=1" />
    <title>Launching Unified Suite</title>
    <style>
      :root {{
        color-scheme: dark;
        --bg: #08101a;
        --panel: rgba(17,25,39,.92);
        --panel-soft: rgba(77,155,255,.08);
        --border: rgba(77,155,255,.18);
        --text: #e8eef7;
        --muted: #91a4ba;
        --accent: #4d9bff;
      }}
      * {{ box-sizing: border-box; }}
      body {{
        margin: 0;
        min-height: 100vh;
        display: grid;
        place-items: center;
        padding: 32px;
        font-family: "Segoe UI", system-ui, sans-serif;
        background:
          radial-gradient(circle at top, rgba(77,155,255,.14), transparent 38%),
          linear-gradient(180deg, #071019 0%, #0d1520 100%);
        color: var(--text);
      }}
      .shell {{
        width: min(780px, 100%);
        background: var(--panel);
        border: 1px solid var(--border);
        border-radius: 22px;
        padding: 28px;
        box-shadow: 0 30px 80px rgba(0,0,0,.35);
      }}
      .eyebrow {{
        display: inline-flex;
        align-items: center;
        gap: 10px;
        padding: 6px 10px;
        border-radius: 999px;
        background: rgba(77,155,255,.1);
        border: 1px solid rgba(77,155,255,.18);
        color: #bfdcff;
        font-size: 12px;
        letter-spacing: .08em;
        text-transform: uppercase;
      }}
      .spinner {{
        width: 11px;
        height: 11px;
        border-radius: 999px;
        background: var(--accent);
        box-shadow: 0 0 0 0 rgba(77,155,255,.75);
        animation: pulse 1.25s infinite;
      }}
      @keyframes pulse {{
        0% {{ box-shadow: 0 0 0 0 rgba(77,155,255,.6); }}
        70% {{ box-shadow: 0 0 0 16px rgba(77,155,255,0); }}
        100% {{ box-shadow: 0 0 0 0 rgba(77,155,255,0); }}
      }}
      h1 {{ margin: 18px 0 10px; font-size: 32px; }}
      p {{ margin: 0; color: var(--muted); line-height: 1.6; }}
      .panel {{
        margin-top: 20px;
        padding: 16px 18px;
        border-radius: 16px;
        background: var(--panel-soft);
        border: 1px solid rgba(77,155,255,.12);
      }}
      .meta {{
        display: grid;
        gap: 10px;
        margin-top: 18px;
      }}
      .meta div {{
        display: flex;
        justify-content: space-between;
        gap: 12px;
        padding: 12px 14px;
        border-radius: 14px;
        background: rgba(255,255,255,.03);
        border: 1px solid rgba(255,255,255,.05);
      }}
      .meta span:first-child {{
        color: var(--muted);
      }}
      .actions {{
        display: flex;
        flex-wrap: wrap;
        gap: 10px;
        margin-top: 20px;
      }}
      .steps {{
        display: grid;
        gap: 8px;
        margin-top: 18px;
      }}
      .step {{
        display: flex;
        align-items: center;
        justify-content: space-between;
        gap: 12px;
        padding: 10px 12px;
        border-radius: 12px;
        background: rgba(255,255,255,.03);
        border: 1px solid rgba(255,255,255,.05);
        color: var(--muted);
        font-size: 13px;
      }}
      .step strong {{
        color: var(--text);
      }}
      button {{
        cursor: pointer;
        border: 1px solid transparent;
        border-radius: 12px;
        padding: 12px 16px;
        font: inherit;
        font-weight: 600;
        background: var(--accent);
        color: white;
      }}
      button.secondary {{
        background: transparent;
        border-color: rgba(255,255,255,.1);
        color: var(--text);
      }}
      code {{
        font-family: "Cascadia Mono", Consolas, monospace;
        color: var(--text);
        word-break: break-all;
      }}
    </style>
  </head>
  <body>
    <section class="shell">
      <span class="eyebrow"><span class="spinner"></span> SIEM-Lite Unified Suite</span>
      <h1>Bringing the portal online</h1>
      <p>The desktop shell is live already. While the portal becomes healthy, this window stays responsive and gives you recovery actions.</p>
      <div class="panel">{status}</div>
      <div class="meta">
        <div><span>Target URL</span><code>{url}</code></div>
        <div><span>Shortcut</span><strong>F5 / Ctrl+R to retry or reload</strong></div>
        <div><span>Session timer</span><strong id="boot-elapsed">0s</strong></div>
      </div>
      <div class="steps">
        <div class="step"><span>Window shell</span><strong>Ready</strong></div>
        <div class="step"><span>Portal health check</span><strong id="boot-stage">Running</strong></div>
        <div class="step"><span>Recovery path</span><strong>Browser fallback available</strong></div>
      </div>
      <div class="actions">
        <button type="button" onclick="window.ipc.postMessage('retry')">Retry startup</button>
        <button type="button" class="secondary" onclick="window.ipc.postMessage('open-external')">Open in browser</button>
        <button type="button" class="secondary" onclick="navigator.clipboard && navigator.clipboard.writeText('{url}')">Copy URL</button>
      </div>
      <script>
        const startedAt = Date.now();
        const stage = document.getElementById('boot-stage');
        const elapsed = document.getElementById('boot-elapsed');
        const steps = ['Checking portal health', 'Starting local portal', 'Preparing unified suite'];
        let idx = 0;
        setInterval(() => {{
          if (elapsed) {{
            elapsed.textContent = Math.max(1, Math.round((Date.now() - startedAt) / 1000)) + 's';
          }}
          if (stage) {{
            stage.textContent = steps[idx % steps.length];
            idx += 1;
          }}
        }}, 1000);
      </script>
    </section>
  </body>
</html>"#
    )
}

fn error_screen_html(url: &str, message: &str) -> String {
    let url = escape_html(url);
    let message = escape_html(message).replace('\n', "<br />");
    format!(
        r#"<!doctype html>
<html lang="en">
  <head>
    <meta charset="utf-8" />
    <meta name="viewport" content="width=device-width,initial-scale=1" />
    <title>Portal unavailable</title>
    <style>
      :root {{
        color-scheme: dark;
        --bg: #08101a;
        --panel: rgba(17,25,39,.94);
        --border: rgba(248,81,73,.2);
        --text: #e8eef7;
        --muted: #91a4ba;
        --critical: #f85149;
        --critical-soft: rgba(248,81,73,.1);
      }}
      * {{ box-sizing: border-box; }}
      body {{
        margin: 0;
        min-height: 100vh;
        display: grid;
        place-items: center;
        padding: 32px;
        font-family: "Segoe UI", system-ui, sans-serif;
        background:
          radial-gradient(circle at top, rgba(248,81,73,.16), transparent 36%),
          linear-gradient(180deg, #071019 0%, #0d1520 100%);
        color: var(--text);
      }}
      .shell {{
        width: min(820px, 100%);
        background: var(--panel);
        border: 1px solid var(--border);
        border-radius: 22px;
        padding: 28px;
        box-shadow: 0 30px 80px rgba(0,0,0,.35);
      }}
      .eyebrow {{
        display: inline-flex;
        align-items: center;
        padding: 6px 10px;
        border-radius: 999px;
        background: var(--critical-soft);
        border: 1px solid rgba(248,81,73,.2);
        color: #ffb4b1;
        font-size: 12px;
        letter-spacing: .08em;
        text-transform: uppercase;
      }}
      h1 {{ margin: 18px 0 10px; font-size: 32px; }}
      p {{ margin: 0; color: var(--muted); line-height: 1.6; }}
      .error {{
        margin-top: 20px;
        padding: 16px 18px;
        border-radius: 16px;
        background: var(--critical-soft);
        border: 1px solid rgba(248,81,73,.16);
        color: #ffd7d5;
        line-height: 1.6;
      }}
      .meta {{
        display: grid;
        gap: 10px;
        margin-top: 18px;
      }}
      .meta div {{
        display: flex;
        justify-content: space-between;
        gap: 12px;
        padding: 12px 14px;
        border-radius: 14px;
        background: rgba(255,255,255,.03);
        border: 1px solid rgba(255,255,255,.05);
      }}
      .meta span:first-child {{
        color: var(--muted);
      }}
      .actions {{
        display: flex;
        flex-wrap: wrap;
        gap: 10px;
        margin-top: 20px;
      }}
      button {{
        cursor: pointer;
        border: 1px solid transparent;
        border-radius: 12px;
        padding: 12px 16px;
        font: inherit;
        font-weight: 600;
        background: var(--critical);
        color: white;
      }}
      button.secondary {{
        background: transparent;
        border-color: rgba(255,255,255,.1);
        color: var(--text);
      }}
      code {{
        font-family: "Cascadia Mono", Consolas, monospace;
        color: var(--text);
        word-break: break-all;
      }}
    </style>
  </head>
  <body>
    <section class="shell">
      <span class="eyebrow">Portal unavailable</span>
      <h1>The desktop shell could not reach the suite</h1>
      <p>The operator window is still healthy. You can retry startup, or open the target URL in an external browser for direct troubleshooting.</p>
      <div class="error">{message}</div>
      <div class="meta">
        <div><span>Target URL</span><code>{url}</code></div>
        <div><span>Hint</span><strong>Check Docker/services, firewall, auth, and SIEM_OPERATOR_PORTAL_URL</strong></div>
      </div>
      <div class="actions">
        <button type="button" onclick="window.ipc.postMessage('retry')">Retry startup</button>
        <button type="button" class="secondary" onclick="window.ipc.postMessage('open-external')">Open in browser</button>
        <button type="button" class="secondary" onclick="navigator.clipboard && navigator.clipboard.writeText('{url}')">Copy URL</button>
      </div>
    </section>
  </body>
</html>"#
    )
}

fn start_portal_bootstrap(url: String, proxy: EventLoopProxy<UserEvent>) {
    std::thread::spawn(move || {
        let result = ensure_portal_available(&url).map_err(|err| err.to_string());
        report_if_err(
            proxy.send_event(UserEvent::PortalBootstrapFinished(result)),
            "failed to post bootstrap result event",
        );
    });
}

/// Окно WebView → SIEM Portal (нужен WebView2 / webkit2gtk).
pub fn run_portal_webview() -> wry::Result<()> {
    let url = portal_url();
    let event_loop = EventLoopBuilder::<UserEvent>::with_user_event().build();
    let proxy = event_loop.create_proxy();
    let window = WindowBuilder::new()
        .with_title(&operator_window_title(Some("Launching Unified Suite")))
        .with_inner_size(LogicalSize::new(1360.0, 860.0))
        .with_min_inner_size(LogicalSize::new(1040.0, 640.0))
        .build(&event_loop)
        .map_err(|e| wry::Error::Io(std::io::Error::other(e)))?;

    let ipc_proxy = proxy.clone();
    let title_proxy = proxy.clone();
    let builder = WebViewBuilder::new()
        .with_html(loading_screen_html(
            &url,
            "Checking whether SIEM Portal is already healthy and starting it automatically when possible.",
        ))
        .with_ipc_handler(move |req| match req.body().as_str() {
            "retry" => {
                report_if_err(
                    ipc_proxy.send_event(UserEvent::RetryPortalBootstrap),
                    "failed to post retry event",
                );
            }
            "open-external" => {
                report_if_err(
                    ipc_proxy.send_event(UserEvent::OpenPortalInBrowser),
                    "failed to post open-external event",
                );
            }
            _ => {}
        })
        .with_document_title_changed_handler(move |title| {
            report_if_err(
                title_proxy.send_event(UserEvent::UpdateTitle(title)),
                "failed to post title-update event",
            );
        });

    #[cfg(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    ))]
    let webview = builder.build(&window)?;

    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    )))]
    let webview = {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        let vbox = window
            .default_vbox()
            .ok_or_else(|| wry::Error::Io(std::io::Error::other("no GTK vbox")))?;
        builder.build_gtk(vbox)?
    };

    let mut managed_portal: Option<Child> = None;
    let mut modifiers = ModifiersState::empty();
    let mut boot_in_progress = true;
    let mut portal_loaded = false;
    let bootstrap_proxy = proxy.clone();
    start_portal_bootstrap(url.clone(), bootstrap_proxy);

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        match event {
            Event::UserEvent(UserEvent::PortalBootstrapFinished(result)) => {
                boot_in_progress = false;
                match result {
                    Ok(child) => {
                        managed_portal = child;
                        portal_loaded = true;
                        window.set_title(&operator_window_title(Some("Unified Suite")));
                        report_if_err(webview.load_url(&url), "failed to load portal URL");
                    }
                    Err(message) => {
                        portal_loaded = false;
                        window.set_title(&operator_window_title(Some("Portal unavailable")));
                        report_if_err(
                            webview.load_html(&error_screen_html(&url, &message)),
                            "failed to render portal error screen",
                        );
                    }
                }
            }
            Event::UserEvent(UserEvent::RetryPortalBootstrap) => {
                if boot_in_progress {
                    return;
                }
                if let Some(child) = managed_portal.as_mut() {
                    let _ = child.kill();
                }
                managed_portal = None;
                portal_loaded = false;
                boot_in_progress = true;
                window.set_title(&operator_window_title(Some("Launching Unified Suite")));
                report_if_err(
                    webview.load_html(&loading_screen_html(
                        &url,
                        "Retrying portal health check and local auto-start sequence.",
                    )),
                    "failed to render loading screen on retry",
                );
                start_portal_bootstrap(url.clone(), proxy.clone());
            }
            Event::UserEvent(UserEvent::OpenPortalInBrowser) => {
                report_if_err(webbrowser::open(&url), "failed to open portal in browser");
            }
            Event::UserEvent(UserEvent::UpdateTitle(title)) => {
                window.set_title(&operator_window_title(Some(&title)));
            }
            Event::WindowEvent {
                event: WindowEvent::ModifiersChanged(next),
                ..
            } => {
                modifiers = next;
            }
            Event::WindowEvent {
                event: WindowEvent::KeyboardInput { event, .. },
                ..
            } => {
                if event.state != ElementState::Pressed || event.repeat {
                    return;
                }
                let reload_pressed = event.physical_key == KeyCode::F5
                    || ((modifiers.control_key() || modifiers.super_key())
                        && matches!(
                            event.key_without_modifiers(),
                            Key::Character(ch) if ch.eq_ignore_ascii_case("r")
                        ));
                if reload_pressed {
                    if portal_loaded {
                        report_if_err(webview.reload(), "failed to reload webview");
                    } else if !boot_in_progress {
                        report_if_err(
                            proxy.send_event(UserEvent::RetryPortalBootstrap),
                            "failed to post retry event from keyboard shortcut",
                        );
                    }
                }
            }
            Event::WindowEvent {
                event: WindowEvent::CloseRequested,
                ..
            } => {
                if let Some(child) = managed_portal.as_mut() {
                    let _ = child.kill();
                }
                *control_flow = ControlFlow::Exit;
            }
            _ => {}
        }
    });
}
