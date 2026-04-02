package engine

import (
	"github.com/prometheus/client_golang/prometheus"
	"github.com/prometheus/client_golang/prometheus/promauto"
)

type EngineMetrics struct {
	eventsProcessed prometheus.Counter
	parseErrors     prometheus.Counter
	alertsFired     *prometheus.CounterVec
	alertsDropped   prometheus.Counter
	processDuration prometheus.Histogram
}

func newEngineMetrics() *EngineMetrics {
	return &EngineMetrics{
		eventsProcessed: promauto.NewCounter(prometheus.CounterOpts{
			Namespace: "detection",
			Name:      "events_processed_total",
			Help:      "Total number of events processed by the detection engine",
		}),
		parseErrors: promauto.NewCounter(prometheus.CounterOpts{
			Namespace: "detection",
			Name:      "parse_errors_total",
			Help:      "Total number of events that failed JSON deserialization",
		}),
		alertsFired: promauto.NewCounterVec(prometheus.CounterOpts{
			Namespace: "detection",
			Name:      "alerts_fired_total",
			Help:      "Total number of alerts fired, partitioned by severity and rule_id",
		}, []string{"severity", "rule_id"}),
		alertsDropped: promauto.NewCounter(prometheus.CounterOpts{
			Namespace: "detection",
			Name:      "alerts_dropped_total",
			Help:      "Total number of alerts dropped due to full channel",
		}),
		processDuration: promauto.NewHistogram(prometheus.HistogramOpts{
			Namespace: "detection",
			Name:      "process_duration_seconds",
			Help:      "Time to process a single event through all rules",
			Buckets:   prometheus.DefBuckets,
		}),
	}
}
