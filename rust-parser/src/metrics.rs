//! Prometheus-метрики для siem-parser.
//! Экспортируются на :9100/metrics.

use once_cell::sync::Lazy;
use prometheus::{
    register_histogram_vec, register_int_counter_vec, register_int_gauge, HistogramVec,
    IntCounterVec, IntGauge,
};

pub static EVENTS_PARSED_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "siem_parser_events_parsed_total",
        "Total number of events parsed",
        &["source_type", "format", "status"]
    )
    .expect("metric registration failed")
});

pub static SIEM_EVENTS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "siem_events_total",
        "Normalized SIEM events for security alert rules",
        &[
            "source_type",
            "severity",
            "status_code",
            "url_path",
            "source_ip"
        ]
    )
    .expect("metric registration failed")
});

pub static PARSE_DURATION_SECONDS: Lazy<HistogramVec> = Lazy::new(|| {
    register_histogram_vec!(
        "siem_parser_parse_duration_seconds",
        "Time spent parsing and normalizing a single event",
        &["source_type"],
        // Бакеты: 0.5ms, 1ms, 2ms, 5ms, 10ms, 25ms, 50ms
        vec![0.0005, 0.001, 0.002, 0.005, 0.010, 0.025, 0.050]
    )
    .expect("metric registration failed")
});

pub static PII_MASKS_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "siem_parser_pii_masks_total",
        "Total PII masks applied by type",
        &["pii_type"]
    )
    .expect("metric registration failed")
});

pub static EVENTS_IN_FLIGHT: Lazy<IntGauge> = Lazy::new(|| {
    register_int_gauge!(
        "siem_parser_events_in_flight",
        "Number of events currently being processed"
    )
    .expect("metric registration failed")
});

pub static KAFKA_PRODUCED_TOTAL: Lazy<IntCounterVec> = Lazy::new(|| {
    register_int_counter_vec!(
        "siem_parser_kafka_produced_total",
        "Total events produced to Kafka",
        &["topic", "status"]
    )
    .expect("metric registration failed")
});

/// Инициализирует метрики (вызывает Lazy::force).
pub fn init() {
    Lazy::force(&EVENTS_PARSED_TOTAL);
    Lazy::force(&SIEM_EVENTS_TOTAL);
    Lazy::force(&PARSE_DURATION_SECONDS);
    Lazy::force(&PII_MASKS_TOTAL);
    Lazy::force(&EVENTS_IN_FLIGHT);
    Lazy::force(&KAFKA_PRODUCED_TOTAL);
}
