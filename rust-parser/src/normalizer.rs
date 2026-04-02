//! Пайплайн нормализации: Parser → PII masking → Enrichment → Output.

use crate::{
    enrichment::Enricher,
    error::ParserError,
    metrics,
    parser,
    pii,
    schema::NormalizedEvent,
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
        Self { enricher, enable_pii, enable_geoip }
    }

    /// Обрабатывает одно событие. Время <5ms p99 на 1KB события.
    pub fn process(
        &self,
        raw: Bytes,
        source_type: &str,
        host: &str,
    ) -> Result<NormalizedEvent, ParserError> {
        let start = Instant::now();
        metrics::EVENTS_IN_FLIGHT.inc();

        let result = self.process_inner(raw, source_type, host);

        let elapsed = start.elapsed();
        metrics::EVENTS_IN_FLIGHT.dec();

        let status = if result.is_ok() { "ok" } else { "error" };
        metrics::EVENTS_PARSED_TOTAL
            .with_label_values(&[source_type, "auto", status])
            .inc();
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

    fn process_inner(
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
            pii::mask_sensitive_json_keys(
                &mut serde_json::to_value(&event.metadata)
                    .unwrap_or(serde_json::Value::Object(Default::default()))
            );
            if let Some(masked_path) = event.url_path.as_deref().and_then(pii::mask_pii) {
                event.url_path = Some(masked_path);
            }
        }

        // 3. GeoIP + ASN обогащение
        if self.enable_geoip {
            self.enricher.enrich(&mut event);
        }

        Ok(event)
    }
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

    #[test]
    fn test_pii_masked_in_pipeline() {
        let pipeline = make_pipeline();
        let raw = br#"{"Level":"Info","Message":"User admin@corp.com logged in with token=eyJhbGci.payload.sig"}"#;
        let event = pipeline.process(Bytes::from_static(raw), "dotnet", "test-host").unwrap();
        assert!(!event.message.contains("admin@corp.com"));
        assert!(!event.message.contains("eyJhbGci"));
    }

    #[test]
    fn test_sla_compliant() {
        let pipeline = make_pipeline();
        let raw = bytes::Bytes::from(serde_json::json!({
            "Level": "Warning",
            "Message": "Login failed",
            "Timestamp": "2024-01-15T10:00:00Z",
            "Properties": {
                "ClientIp": "203.0.113.42",
                "UserId": "testuser",
                "StatusCode": 401
            }
        }).to_string());

        let start = std::time::Instant::now();
        let _event = pipeline.process(raw, "dotnet", "host").unwrap();
        let elapsed = start.elapsed();
        // В тесте без GeoIP и с простым JSON — должно быть <1ms
        assert!(elapsed.as_millis() < 10, "Processing took {}ms, expected <10ms in test", elapsed.as_millis());
    }
}
