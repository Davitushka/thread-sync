use prometheus::{Counter, CounterVec, Histogram, HistogramOpts, Opts};

pub struct EngineMetrics {
    pub events_processed: Counter,
    pub parse_errors: Counter,
    pub alerts_fired: CounterVec,
    pub alerts_dropped: Counter,
    pub process_duration: Histogram,
}

impl EngineMetrics {
    pub fn new() -> Self {
        let events_processed = Counter::with_opts(Opts::new(
            "detection_events_processed_total",
            "Total number of events processed by the detection engine",
        ))
        .unwrap();
        prometheus::register(Box::new(events_processed.clone())).unwrap();

        let parse_errors = Counter::with_opts(Opts::new(
            "detection_parse_errors_total",
            "Total number of events that failed JSON deserialization",
        ))
        .unwrap();
        prometheus::register(Box::new(parse_errors.clone())).unwrap();

        let alerts_fired = CounterVec::new(
            Opts::new(
                "detection_alerts_fired_total",
                "Total number of alerts fired, partitioned by severity and rule_id",
            ),
            &["severity", "rule_id"],
        )
        .unwrap();
        prometheus::register(Box::new(alerts_fired.clone())).unwrap();

        let alerts_dropped = Counter::with_opts(Opts::new(
            "detection_alerts_dropped_total",
            "Total number of alerts dropped due to full channel",
        ))
        .unwrap();
        prometheus::register(Box::new(alerts_dropped.clone())).unwrap();

        let process_duration = Histogram::with_opts(HistogramOpts::new(
            "detection_process_duration_seconds",
            "Time to process a single event through all rules",
        ))
        .unwrap();
        prometheus::register(Box::new(process_duration.clone())).unwrap();

        Self {
            events_processed,
            parse_errors,
            alerts_fired,
            alerts_dropped,
            process_duration,
        }
    }
}
