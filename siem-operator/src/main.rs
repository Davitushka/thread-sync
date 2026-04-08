//! Один exe: по умолчанию egui; `cargo run -- --web` или `SIEM_OPERATOR_MODE=portal` — WebView с порталом.
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

  cargo run                    — нативный UI (egui)
  cargo run -- --web           — окно с SIEM Portal в WebView (http://127.0.0.1:8091/)
  cargo run -- --help          — эта справка

  SIEM_OPERATOR_MODE=portal     — как --web (в PowerShell: $env:SIEM_OPERATOR_MODE='portal')

  SIEM_OPERATOR_PORTAL_URL      — URL портала для WebView
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

    if portal_mode_from_args() || portal_mode_from_env() {
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
