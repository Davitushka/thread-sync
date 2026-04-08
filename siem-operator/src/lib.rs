//! SIEM-Lite Operator: модули + egui + опционально WebView к порталу (один exe).
pub mod app;
pub mod models;
pub mod theme;
pub mod ui;

use eframe::egui;
use tao::{
    dpi::LogicalSize,
    event::{Event, WindowEvent},
    event_loop::{ControlFlow, EventLoop},
    window::WindowBuilder,
};
use wry::WebViewBuilder;

/// URL портала для окна WebView (`--web`).
pub fn portal_url() -> String {
    let raw = std::env::var("SIEM_OPERATOR_PORTAL_URL")
        .unwrap_or_else(|_| "http://127.0.0.1:8091/".to_string());
    let s = raw.trim().to_string();
    if s.is_empty() {
        "http://127.0.0.1:8091/".to_string()
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

/// Окно WebView → SIEM Portal (нужен WebView2 / webkit2gtk).
pub fn run_portal_webview() -> wry::Result<()> {
    let url = portal_url();

    let event_loop = EventLoop::new();
    let window = WindowBuilder::new()
        .with_title("SIEM-Lite Operator · Portal")
        .with_inner_size(LogicalSize::new(1180.0, 760.0))
        .with_min_inner_size(LogicalSize::new(900.0, 560.0))
        .build(&event_loop)
        .map_err(|e| wry::Error::Io(std::io::Error::new(std::io::ErrorKind::Other, e)))?;

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
        let vbox = window.default_vbox().ok_or_else(|| {
            wry::Error::Io(std::io::Error::new(
                std::io::ErrorKind::Other,
                "no GTK vbox",
            ))
        })?;
        builder.build_gtk(vbox)?
    };

    event_loop.run(move |event, _, control_flow| {
        *control_flow = ControlFlow::Wait;
        if let Event::WindowEvent {
            event: WindowEvent::CloseRequested,
            ..
        } = event
        {
            *control_flow = ControlFlow::Exit;
        }
    });
}
