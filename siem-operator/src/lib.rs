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
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use wry::WebViewBuilder;

const DEFAULT_PORTAL_URL: &str = "http://127.0.0.1:8091/";

/// URL портала для окна WebView (`--web`).
pub fn portal_url() -> String {
    let raw = std::env::var("SIEM_OPERATOR_PORTAL_URL")
        .unwrap_or_else(|_| DEFAULT_PORTAL_URL.to_string());
    let s = raw.trim().to_string();
    if s.is_empty() {
        DEFAULT_PORTAL_URL.to_string()
    } else {
        s
    }
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

fn portal_health_url(raw: &str) -> Option<String> {
    let mut url = Url::parse(raw).ok()?;
    url.set_path("/health");
    url.set_query(None);
    url.set_fragment(None);
    Some(url.to_string())
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
    let Some(health_url) = portal_health_url(raw) else {
        return false;
    };
    let Ok(client) = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
    else {
        return false;
    };
    client
        .get(health_url)
        .send()
        .map(|resp| resp.status().is_success())
        .unwrap_or(false)
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
        repo_root.join("siem-portal").join("target").join("release").join(exe),
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
    set_default_portal_env(cmd, "SIEM_PORTAL_PUBLIC_PROMETHEUS", "http://127.0.0.1:9090");
    set_default_portal_env(cmd, "SIEM_PORTAL_PUBLIC_ALERTMANAGER", "http://127.0.0.1:9093");
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

fn ensure_local_portal_available(raw: &str) -> std::io::Result<Option<Child>> {
    if !portal_autostart_enabled() || !portal_is_local(raw) || portal_ready(raw, Duration::from_secs(2)) {
        return Ok(None);
    }

    let repo_root = locate_repo_root().ok_or_else(|| {
        std::io::Error::new(
            std::io::ErrorKind::NotFound,
            "Не удалось найти репозиторий рядом с siem-operator для автозапуска siem-portal",
        )
    })?;
    let child = spawn_portal_process(&repo_root, raw)?;

    if wait_for_portal(raw, Duration::from_secs(30)) {
        Ok(Some(child))
    } else {
        Err(std::io::Error::other(
            "siem-operator запустил siem-portal, но портал не поднялся за 30 секунд",
        ))
    }
}

/// Окно WebView → SIEM Portal (нужен WebView2 / webkit2gtk).
pub fn run_portal_webview() -> wry::Result<()> {
    let url = portal_url();
    let managed_portal = ensure_local_portal_available(&url).map_err(wry::Error::Io)?;

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("SIEM-Lite Operator · Portal")
        .with_inner_size(LogicalSize::new(1180.0, 760.0))
        .with_min_inner_size(LogicalSize::new(900.0, 560.0))
        .build(&event_loop)
        .map_err(|e| wry::Error::Io(std::io::Error::other(e)))?;

    let builder = WebViewBuilder::new().with_url(&url);

    #[cfg(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    ))]
    let _webview = builder.build(&window)?;

    #[cfg(not(any(
        target_os = "windows",
        target_os = "macos",
        target_os = "ios",
        target_os = "android"
    )))]
    let _webview = {
        use tao::platform::unix::WindowExtUnix;
        use wry::WebViewBuilderExtUnix;
        let vbox = window
            .default_vbox()
            .ok_or_else(|| wry::Error::Io(std::io::Error::other("no GTK vbox")))?;
        builder.build_gtk(vbox)?
    };

    let mut managed_portal = managed_portal;

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            if let Some(child) = managed_portal.as_mut() {
                let _ = child.kill();
            }
            *control_flow = ControlFlow::Exit;
        }
    });
}
