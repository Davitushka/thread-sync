//! Пайплайн нормализации: Parser → PII masking → Enrichment → Output.

use crate::{
    enrichment::Enricher, error::ParserError, metrics, parser, pii, schema::NormalizedEvent,
};
use bytes::Bytes;
use std::time::Instant;
use tracing::warn;

pub struct NormalizationPipeline {
    enricher: Enricher,
    enable_pii: bool,
    enable_geoip: bool,
}

impl NormalizationPipeline {
    pub fn new(enricher: Enricher, enable_pii: bool, enable_geoip: bool) -> Self {
        Self {
            enricher,
            enable_pii,
            enable_geoip,
        }
    }

    /// Обрабатывает одно событие. Время <5ms p99 на 1KB события.
    pub async fn process(
        &self,
        raw: Bytes,
        source_type: &str,
        host: &str,
    ) -> Result<NormalizedEvent, ParserError> {
        let start = Instant::now();
        metrics::EVENTS_IN_FLIGHT.inc();

        let result = self.process_inner(raw, source_type, host).await;

        let elapsed = start.elapsed();
        metrics::EVENTS_IN_FLIGHT.dec();

        let status = if result.is_ok() { "ok" } else { "error" };
        metrics::EVENTS_PARSED_TOTAL
            .with_label_values(&[source_type, "auto", status])
            .inc();
        if let Ok(event) = &result {
            // Zero-alloc label construction for the hot path:
            // - status_code: itoa formats u16 into a stack buffer, no heap
            // - severity: as_str() returns &'static str
            // - url_path: borrowed from event or metadata, Cow avoids clone
            let mut status_code_buf = itoa::Buffer::new();
            let status_code = event
                .status_code
                .map(|v| status_code_buf.format(v))
                .unwrap_or("none");
            let severity = event.severity.as_str();
            let url_path_value = event
                .url_path
                .as_deref()
                .filter(|s| !s.is_empty())
                .or_else(|| metric_url_path_from_metadata(event))
                .or_else(|| metric_url_path_from_message(event))
                .unwrap_or("none");
            let source_ip = event
                .source_ip
                .as_deref()
                .filter(|value| !value.is_empty())
                .unwrap_or("none");
            metrics::SIEM_EVENTS_TOTAL
                .with_label_values(&[
                    event.source_type.as_str(),
                    severity,
                    status_code,
                    url_path_value,
                    source_ip,
                ])
                .inc();
        }
        metrics::PARSE_DURATION_SECONDS
            .with_label_values(&[source_type])
            .observe(elapsed.as_secs_f64());

        // Предупреждение если превышаем SLA
        if elapsed.as_millis() > 5 {
            warn!(
                source_type = source_type,
                elapsed_ms = elapsed.as_millis(),
                "Parse latency exceeded 5ms SLA"
            );
        }

        result
    }

    async fn process_inner(
        &self,
        raw: Bytes,
        source_type: &str,
        host: &str,
    ) -> Result<NormalizedEvent, ParserError> {
        // 1. Парсинг и нормализация структуры
        let mut event = parser::parse(raw, source_type, host)?;

        // 2. PII маскирование (до обогащения, до записи в storage)
        if self.enable_pii {
            if let Some(masked) = pii::mask_pii(&event.message) {
                event.message = masked;
            }
            if let Ok(mut val) = serde_json::to_value(&event.metadata) {
                pii::mask_sensitive_json_keys(&mut val);
                if let Ok(masked) = serde_json::from_value(val) {
                    event.metadata = masked;
                }
            }
            if let Some(masked_path) = event.url_path.as_deref().and_then(pii::mask_pii) {
                event.url_path = Some(masked_path);
            }
        }

        // 3. GeoIP + ASN обогащение
        if self.enable_geoip {
            self.enricher.enrich(&mut event).await;
        }

        Ok(event)
    }
}

/// Extracts the URL path from metadata, returning a borrowed &str when possible.
/// Falls back to an allocated string only when stripping query params.
fn metric_url_path_from_metadata(event: &NormalizedEvent) -> Option<&str> {
    event
        .metadata
        .get("RequestPath")
        .or_else(|| event.metadata.get("Path"))
        .or_else(|| event.metadata.get("Url"))
        .and_then(|value| value.as_str())
        .map(|value| value.split('?').next().unwrap_or(value))
}

fn metric_url_path_from_message(event: &NormalizedEvent) -> Option<&str> {
    event.message.split_whitespace().find_map(|token| {
        let trimmed = token
            .trim_matches(|c: char| matches!(c, '"' | '\'' | ',' | ';' | '(' | ')' | '[' | ']'));
        if trimmed.starts_with('/') {
            Some(trimmed.split('?').next().unwrap_or(trimmed))
        } else {
            None
        }
    })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::enrichment::{Enricher, EnrichmentConfig};

    fn make_pipeline() -> NormalizationPipeline {
        let enricher = Enricher::new(&EnrichmentConfig {
            geoip_city_db_path: "/nonexistent".to_string(),
            geoip_asn_db_path: "/nonexistent".to_string(),
            ..Default::default()
        });
        NormalizationPipeline::new(enricher, true, false)
    }

    #[tokio::test]
    async fn test_pii_masked_in_pipeline() {
        let pipeline = make_pipeline();
        let raw = br#"{"Level":"Info","Message":"User admin@corp.com logged in with token=eyJhbGci.payload.sig"}"#;
        let event = pipeline
            .process(Bytes::from_static(raw), "dotnet", "test-host")
            .await
            .unwrap();
        assert!(!event.message.contains("admin@corp.com"));
        assert!(!event.message.contains("eyJhbGci"));
    }

    #[tokio::test]
    async fn test_sla_compliant() {
        let pipeline = make_pipeline();
        let raw = bytes::Bytes::from(
            serde_json::json!({
                "Level": "Warning",
                "Message": "Login failed",
                "Timestamp": "2024-01-15T10:00:00Z",
                "Properties": {
                    "ClientIp": "203.0.113.42",
                    "UserId": "testuser",
                    "StatusCode": 401
                }
            })
            .to_string(),
        );

        let start = std::time::Instant::now();
        let _event = pipeline.process(raw, "dotnet", "host").await.unwrap();
        let elapsed = start.elapsed();
        // В тесте без GeoIP и с простым JSON — должно быть <1ms
        assert!(
            elapsed.as_millis() < 10,
            "Processing took {}ms, expected <10ms in test",
            elapsed.as_millis()
        );
    }

    #[test]
    fn metadata_request_path_fallback_is_sanitized() {
        let mut event = crate::schema::NormalizedEvent::new("dotnet");
        event.metadata.insert(
            "RequestPath".to_string(),
            serde_json::Value::String("/api/auth/login?token=secret".to_string()),
        );
        assert_eq!(
            metric_url_path_from_metadata(&event),
            Some("/api/auth/login")
        );
    }

    #[test]
    fn message_request_path_fallback_extracts_path() {
        let mut event = crate::schema::NormalizedEvent::new("dotnet");
        event.message = "Authentication failed on /api/auth/login from 203.0.113.5".to_string();
        assert_eq!(
            metric_url_path_from_message(&event),
            Some("/api/auth/login")
        );
    }
}
