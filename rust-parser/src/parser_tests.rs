//! Расширенные тесты для parser.rs — подключаются через #[path] в parser.rs
use super::*;
use bytes::Bytes;
use crate::schema::Severity;
use crate::error::ParserError;

// ── Детектирование формата ──────────────────────────────────────────────────

#[test]
fn detect_json_object() {
    assert_eq!(detect_format(b"{\"key\":\"val\"}"), LogFormat::Json);
}

#[test]
fn detect_json_with_leading_whitespace() {
    assert_eq!(detect_format(b"  \n{\"key\":\"val\"}"), LogFormat::Json);
}

#[test]
fn detect_cef() {
    assert_eq!(
        detect_format(b"CEF:0|Vendor|Product|1.0|100|Test|5|src=1.2.3.4"),
        LogFormat::Cef
    );
}

#[test]
fn detect_syslog_rfc5424() {
    assert_eq!(
        detect_format(b"<13>1 2024-01-01T00:00:00Z host app - - msg"),
        LogFormat::Syslog5424
    );
}

#[test]
fn detect_syslog_rfc3164() {
    assert_eq!(
        detect_format(b"<13>Jan  1 00:00:00 host app: message"),
        LogFormat::Syslog3164
    );
}

#[test]
fn detect_plaintext() {
    assert_eq!(detect_format(b"plain log message"), LogFormat::PlainText);
}

// ── JSON парсинг ─────────────────────────────────────────────────────────────

#[test]
fn parse_dotnet_serilog_warning() {
    let raw = br#"{
        "Timestamp": "2024-01-15T10:30:00Z",
        "Level": "Warning",
        "Message": "Login failed",
        "Properties": {
            "ClientIp": "203.0.113.42",
            "UserId": "user123",
            "StatusCode": 401,
            "RequestPath": "/api/auth/login",
            "RequestMethod": "POST"
        }
    }"#;
    let event = parse(Bytes::from_static(raw), "dotnet", "api-01").unwrap();
    assert_eq!(event.severity, Severity::Warning);
    assert_eq!(event.message, "Login failed");
    assert_eq!(event.source_ip.as_deref(), Some("203.0.113.42"));
    assert_eq!(event.user_id.as_deref(), Some("user123"));
    assert_eq!(event.status_code, Some(401));
    assert_eq!(event.http_method.as_deref(), Some("POST"));
    assert_eq!(event.url_path.as_deref(), Some("/api/auth/login"));
    assert_eq!(event.source_type, "dotnet");
    assert_eq!(event.host, "api-01");
}

#[test]
fn parse_dotnet_all_severity_mappings() {
    let cases = [
        ("Verbose", Severity::Debug),
        ("Debug", Severity::Debug),
        ("Information", Severity::Info),
        ("Warning", Severity::Warning),
        ("Error", Severity::Error),
        ("Fatal", Severity::Critical),
    ];
    for (level, expected) in cases {
        let raw = format!(
            r#"{{"Level":"{}","Message":"test","Timestamp":"2024-01-01T00:00:00Z"}}"#,
            level
        );
        let event = parse(Bytes::from(raw), "dotnet", "h").unwrap();
        assert_eq!(event.severity, expected, "Failed for level={}", level);
    }
}

#[test]
fn parse_json_missing_timestamp_uses_now() {
    let raw = br#"{"Level":"Info","Message":"no timestamp"}"#;
    let event = parse(Bytes::from_static(raw), "dotnet", "host").unwrap();
    let diff = (chrono::Utc::now() - event.timestamp).num_seconds().abs();
    assert!(diff < 5, "timestamp diff {} sec is too large", diff);
}

#[test]
fn parse_json_http_fields() {
    let raw = br#"{
        "Level": "Info",
        "Message": "Request",
        "RequestMethod": "DELETE",
        "RequestPath": "/api/users/42?token=secret",
        "StatusCode": 204,
        "Elapsed": 12.5
    }"#;
    let event = parse(Bytes::from_static(raw), "dotnet", "host").unwrap();
    assert_eq!(event.http_method.as_deref(), Some("DELETE"));
    assert_eq!(event.url_path.as_deref(), Some("/api/users/42"));
    assert_eq!(event.status_code, Some(204));
    assert_eq!(event.duration_ms, Some(12.5));
}

#[test]
fn parse_json_non_object_wrapped() {
    let raw = b"\"just a string\"";
    let event = parse(Bytes::from_static(raw), "dotnet", "host").unwrap();
    assert_eq!(event.event_type, "raw");
}

#[test]
fn parse_json_invalid_returns_error() {
    let raw = b"{invalid json{{";
    let result = parse(Bytes::from_static(raw), "dotnet", "host");
    assert!(result.is_err());
}

// ── CEF парсинг ──────────────────────────────────────────────────────────────

#[test]
fn parse_cef_basic() {
    let raw = b"CEF:0|Acme|Firewall|1.0|100|Port Scan Detected|7|src=10.0.0.1 suser=admin request=/admin";
    let event = parse(Bytes::from_static(raw), "firewall", "fw-01").unwrap();
    assert_eq!(event.event_type, "network");
    assert_eq!(event.severity, Severity::Error);
    assert_eq!(event.message, "Port Scan Detected");
}

#[test]
fn parse_cef_severity_mapping() {
    let cases = [
        (0u8, Severity::Info),
        (4, Severity::Warning),
        (7, Severity::Error),
        (9, Severity::Critical),
    ];
    for (sev, expected) in cases {
        let raw = format!("CEF:0|V|P|1|100|Name|{}|src=1.2.3.4", sev);
        let event = parse(Bytes::from(raw), "test", "host").unwrap();
        assert_eq!(event.severity, expected, "CEF severity {} failed", sev);
    }
}

#[test]
fn parse_cef_too_few_fields_returns_error() {
    let raw = b"CEF:0|Vendor|Product";
    let result = parse(Bytes::from_static(raw), "test", "host");
    assert!(result.is_err());
}

// ── Syslog RFC5424 ───────────────────────────────────────────────────────────

#[test]
fn parse_syslog5424_basic() {
    let raw = b"<34>1 2024-01-15T10:30:00Z myhost sshd 1234 - Failed password for root";
    let event = parse(Bytes::from_static(raw), "syslog", "host").unwrap();
    assert_eq!(event.event_type, "syslog");
    assert_eq!(event.host, "myhost");
}

// ── Plaintext ────────────────────────────────────────────────────────────────

#[test]
fn parse_plaintext_preserves_message() {
    let msg = "some plain log line without structure";
    let event = parse(Bytes::from(msg), "app", "host").unwrap();
    assert_eq!(event.message, msg);
    assert_eq!(event.event_type, "raw");
}

// ── Размер события ───────────────────────────────────────────────────────────

#[test]
fn parse_max_size_boundary() {
    // Ровно на границе — должен пройти (plaintext)
    let at_limit = vec![b'x'; MAX_EVENT_SIZE];
    let result = parse(Bytes::from(at_limit), "test", "host");
    assert!(result.is_ok());
}

// ── URL sanitization ─────────────────────────────────────────────────────────

#[test]
fn url_query_string_stripped() {
    let raw = br#"{"Level":"Info","Message":"req","RequestPath":"/api/search?q=admin&token=secret123","Timestamp":"2024-01-01T00:00:00Z"}"#;
    let event = parse(Bytes::from_static(raw), "dotnet", "host").unwrap();
    assert_eq!(event.url_path.as_deref(), Some("/api/search"));
    assert!(!event.url_path.as_deref().unwrap_or("").contains("secret"));
}

// ── Source type и host ───────────────────────────────────────────────────────

#[test]
fn source_type_and_host_propagated() {
    let raw = br#"{"Level":"Info","Message":"test","Timestamp":"2024-01-01T00:00:00Z"}"#;
    let event = parse(Bytes::from_static(raw), "postgresql", "db-server-01").unwrap();
    assert_eq!(event.source_type, "postgresql");
    assert_eq!(event.host, "db-server-01");
}

// ── Event ID генерация ────────────────────────────────────────────────────────

#[test]
fn each_event_gets_unique_id() {
    let raw = br#"{"Level":"Info","Message":"test","Timestamp":"2024-01-01T00:00:00Z"}"#;
    let e1 = parse(Bytes::from_static(raw), "dotnet", "host").unwrap();
    let e2 = parse(Bytes::from_static(raw), "dotnet", "host").unwrap();
    assert_ne!(e1.event_id, e2.event_id);
}
