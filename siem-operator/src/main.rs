//! Один exe: по умолчанию WebView с Unified Suite; `--native` или `SIEM_OPERATOR_MODE=native` — legacy egui fallback.
#![cfg_attr(not(debug_assertions), windows_subsystem = "windows")]

fn portal_mode_from_env() -> bool {
    std::env::var("SIEM_OPERATOR_MODE")
        .map(|v| {
            let v = v.trim();
            v.eq_ignore_ascii_case("portal")
                || v.eq_ignore_ascii_case("web")
                || v.eq_ignore_ascii_case("webview")
        })
        .unwrap_or(false)
}

fn portal_mode_from_args() -> bool {
    std::env::args()
        .skip(1)
        .any(|a| matches!(a.as_str(), "--web" | "--portal" | "-w"))
}

fn print_help() {
    eprintln!(
        "\
SIEM-Lite Operator (один бинарь)

  cargo run                    — окно с Unified Suite в WebView (по умолчанию через SIEM Portal)
  cargo run -- --native        — нативный UI (egui, legacy fallback)
  cargo run -- --help          — эта справка

  F5 / Ctrl+R                  — reload / retry Portal inside WebView shell

  SIEM_OPERATOR_MODE=portal     — как режим WebView / Unified Suite
  SIEM_OPERATOR_MODE=native     — как --native

  SIEM_OPERATOR_PORTAL_URL      — URL портала / Unified Suite для WebView
  SIEM_OPERATOR_AUTOSTART_PORTAL — auto-start локального siem-portal (default: on)
  SIEM_OPERATOR_API           — базовый API для egui
"
    );
}

fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.iter().any(|a| a == "--help" || a == "-h") {
        print_help();
        return;
    }

    let native_mode = args
        .iter()
        .skip(1)
        .any(|a| matches!(a.as_str(), "--native" | "--egui"))
        || std::env::var("SIEM_OPERATOR_MODE")
            .map(|v| v.trim().eq_ignore_ascii_case("native"))
            .unwrap_or(false);

    if !native_mode && (args.len() == 1 || portal_mode_from_args() || portal_mode_from_env()) {
        if let Err(e) = siem_operator::run_portal_webview() {
            eprintln!("WebView / Portal: {e}");
            std::process::exit(1);
        }
        return;
    }

    if let Err(e) = siem_operator::run_egui_operator() {
        eprintln!("Operator (egui): {e}");
        std::process::exit(1);
    }
}
