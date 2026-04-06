//! Integration tests — тестируют полный pipeline через публичный API крейта.
//! Запуск: cargo test --test integration_test

use bytes::Bytes;
use siem_parser::{
    enrichment::{Enricher, EnrichmentConfig},
    normalizer::NormalizationPipeline,
    pii::{mask_pii, mask_pii_owned, mask_sensitive_json_keys},
    schema::Severity,
};

// ── Helpers ──────────────────────────────────────────────────────────────────

fn pipeline_no_geoip() -> NormalizationPipeline {
    let enricher = Enricher::new(&EnrichmentConfig {
        geoip_city_db_path: "/nonexistent/city.mmdb".to_string(),
        geoip_asn_db_path: "/nonexistent/asn.mmdb".to_string(),
        user_cache_size: 100,
        user_cache_ttl_secs: 60,
    });
    NormalizationPipeline::new(enricher, true, false)
}

// ── PII Masking ───────────────────────────────────────────────────────────────

#[test]
fn pii_email_masked() {
    let result = mask_pii("User john.doe@example.com logged in").unwrap();
    assert!(result.contains("***@***.***"));
    assert!(!result.contains("john.doe@example.com"));
}

#[test]
fn pii_multiple_emails_masked() {
    let result = mask_pii("From: alice@corp.com To: bob@acme.org").unwrap();
    assert!(!result.contains("alice@corp.com"));
    assert!(!result.contains("bob@acme.org"));
    assert_eq!(result.matches("***@***.***").count(), 2);
}

#[test]
fn pii_phone_masked() {
    let input = "Contact: +7 (495) 123-4567";
    let result = mask_pii(input).unwrap();
    assert!(result.contains("[PHONE]"));
    assert!(!result.contains("495"));
}

#[test]
fn pii_bearer_token_masked() {
    let input = "Authorization: Bearer eyJhbGciOiJSUzI1NiIsInR5cCI6IkpXVCJ9.payload.sig";
    let result = mask_pii(input).unwrap();
    assert!(result.contains("[REDACTED_TOKEN]"));
    assert!(!result.contains("eyJhbGci"));
}

#[test]
fn pii_no_pii_returns_none() {
    let input = "Normal log message: server started on port 8080";
    assert!(
        mask_pii(input).is_none(),
        "Should return None for clean input"
    );
}

#[test]
fn pii_owned_returns_original_when_clean() {
    let input = "Clean message".to_string();
    let result = mask_pii_owned(input.clone());
    assert_eq!(result, input);
}

#[test]
fn pii_json_sensitive_keys_masked() {
    let mut value = serde_json::json!({
        "username": "alice",
        "password": "SuperSecret!",
        "email": "alice@example.com",
        "nested": {
            "token": "eyJhbGci...",
            "api_key": "sk-1234567890abcdef"
        }
    });
    mask_sensitive_json_keys(&mut value);
    assert_eq!(value["password"], "[REDACTED]");
    assert_eq!(value["nested"]["token"], "[REDACTED]");
    assert_eq!(value["nested"]["api_key"], "[REDACTED]");
    // Несенситивные поля не трогаем
    assert_eq!(value["username"], "alice");
}

#[test]
fn pii_json_nested_array_masked() {
    let mut value = serde_json::json!([
        {"password": "secret1"},
        {"password": "secret2"},
        {"name": "safe"}
    ]);
    mask_sensitive_json_keys(&mut value);
    assert_eq!(value[0]["password"], "[REDACTED]");
    assert_eq!(value[1]["password"], "[REDACTED]");
    assert_eq!(value[2]["name"], "safe");
}

// ── Full Pipeline ─────────────────────────────────────────────────────────────

#[test]
fn pipeline_masks_email_in_message() {
    let pipeline = pipeline_no_geoip();
    let raw = serde_json::json!({
        "Level": "Warning",
        "Message": "Login failed for user admin@company.com from 10.0.0.1",
        "Timestamp": "2024-01-15T10:00:00Z"
    })
    .to_string();

    let event = pipeline
        .process(Bytes::from(raw), "dotnet", "host")
        .unwrap();
    assert!(!event.message.contains("admin@company.com"));
    assert!(event.message.contains("***@***.***"));
}

#[test]
fn pipeline_masks_token_in_message() {
    let pipeline = pipeline_no_geoip();
    let raw = serde_json::json!({
        "Level": "Info",
        "Message": "Request with token=eyJhbGciOiJSUzI1NiJ9.payload.signature received",
        "Timestamp": "2024-01-15T10:00:00Z"
    })
    .to_string();

    let event = pipeline
        .process(Bytes::from(raw), "dotnet", "host")
        .unwrap();
    assert!(!event.message.contains("eyJhbGci"));
}

#[test]
fn pipeline_strips_url_query() {
    let pipeline = pipeline_no_geoip();
    let raw = serde_json::json!({
        "Level": "Info",
        "Message": "HTTP request",
        "RequestPath": "/api/users?password=secret&page=1",
        "Timestamp": "2024-01-15T10:00:00Z"
    })
    .to_string();

    let event = pipeline
        .process(Bytes::from(raw), "dotnet", "host")
        .unwrap();
    assert_eq!(event.url_path.as_deref(), Some("/api/users"));
}

#[test]
fn pipeline_assigns_unique_event_ids() {
    let pipeline = pipeline_no_geoip();
    let raw = serde_json::json!({
        "Level": "Info",
        "Message": "test",
        "Timestamp": "2024-01-15T10:00:00Z"
    })
    .to_string();

    let e1 = pipeline
        .process(Bytes::from(raw.clone()), "dotnet", "host")
        .unwrap();
    let e2 = pipeline
        .process(Bytes::from(raw), "dotnet", "host")
        .unwrap();
    assert_ne!(e1.event_id, e2.event_id);
}

#[test]
fn pipeline_severity_propagated_correctly() {
    let pipeline = pipeline_no_geoip();
    let cases = [
        ("Fatal", Severity::Critical),
        ("Error", Severity::Error),
        ("Warning", Severity::Warning),
        ("Information", Severity::Info),
        ("Debug", Severity::Debug),
    ];

    for (level, expected) in cases {
        let raw = serde_json::json!({
            "Level": level,
            "Message": "test",
            "Timestamp": "2024-01-15T10:00:00Z"
        })
        .to_string();
        let event = pipeline
            .process(Bytes::from(raw), "dotnet", "host")
            .unwrap();
        assert_eq!(
            event.severity, expected,
            "Severity mismatch for level={}",
            level
        );
    }
}

#[test]
fn pipeline_handles_cef_input() {
    let pipeline = pipeline_no_geoip();
    let raw = "CEF:0|Acme|WAF|1.0|100|SQLi Detected|9|src=203.0.113.5 request=/api/search";
    let event = pipeline.process(Bytes::from(raw), "waf", "waf-01").unwrap();
    assert_eq!(event.severity, Severity::Critical);
    assert_eq!(event.event_type, "network");
}

#[test]
fn pipeline_handles_plaintext_gracefully() {
    let pipeline = pipeline_no_geoip();
    let raw = "2024-01-15 10:00:00 ERROR Something went wrong in module X";
    let event = pipeline.process(Bytes::from(raw), "app", "host").unwrap();
    assert_eq!(event.event_type, "raw");
    assert!(!event.message.is_empty());
}

#[test]
fn pipeline_ingest_timestamp_close_to_now() {
    let pipeline = pipeline_no_geoip();
    let raw = serde_json::json!({
        "Level": "Info",
        "Message": "test",
        "Timestamp": "2024-01-15T10:00:00Z"
    })
    .to_string();

    let before = chrono::Utc::now();
    let event = pipeline
        .process(Bytes::from(raw), "dotnet", "host")
        .unwrap();
    let after = chrono::Utc::now();

    assert!(event.ingest_ts >= before);
    assert!(event.ingest_ts <= after);
}

// ── Enrichment (без MMDB) ─────────────────────────────────────────────────────

#[test]
fn enricher_no_mmdb_does_not_fail() {
    let enricher = Enricher::new(&EnrichmentConfig {
        geoip_city_db_path: "/nonexistent/city.mmdb".to_string(),
        geoip_asn_db_path: "/nonexistent/asn.mmdb".to_string(),
        user_cache_size: 100,
        user_cache_ttl_secs: 60,
    });
    let mut event = siem_parser::NormalizedEvent::new("test");
    event.source_ip = Some("8.8.8.8".to_string());
    enricher.enrich(&mut event);
    // Без MMDB — geo должен быть None, но событие не потеряно
    assert!(event.geo.is_none());
    assert_eq!(event.source_ip.as_deref(), Some("8.8.8.8"));
}

#[test]
fn enricher_skips_private_ip() {
    let enricher = Enricher::new(&EnrichmentConfig {
        geoip_city_db_path: "/nonexistent/city.mmdb".to_string(),
        geoip_asn_db_path: "/nonexistent/asn.mmdb".to_string(),
        user_cache_size: 100,
        user_cache_ttl_secs: 60,
    });
    let mut event = siem_parser::NormalizedEvent::new("test");
    event.source_ip = Some("192.168.1.1".to_string());
    enricher.enrich(&mut event);
    // Приватный IP — geo всегда None
    assert!(event.geo.is_none());
}
