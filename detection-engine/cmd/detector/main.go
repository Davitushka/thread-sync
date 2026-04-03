// Команда детектора — запускает Kafka consumer + detection engine + HTTP metrics.
package main

import (
	"context"
	"net/http"
	"os"
	"os/signal"
	"strings"
	"syscall"
	"time"

	"github.com/prometheus/client_golang/prometheus"
	"github.com/prometheus/client_golang/prometheus/promauto"
	"github.com/prometheus/client_golang/prometheus/promhttp"
	kafka "github.com/segmentio/kafka-go"
	"go.uber.org/zap"

	"github.com/siem-lite/detection-engine/internal/correlator"
	"github.com/siem-lite/detection-engine/internal/engine"
	"github.com/siem-lite/detection-engine/internal/rules"
)

var consumerLag = promauto.NewGauge(prometheus.GaugeOpts{
	Name: "detection_kafka_consumer_lag",
	Help: "Approximate consumer lag (messages behind high watermark)",
})

func main() {
	// ── Logger ────────────────────────────────────────────────────────────────
	logger, err := zap.NewProduction()
	if err != nil {
		panic("failed to create logger: " + err.Error())
	}
	defer logger.Sync() //nolint:errcheck

	// ── Конфигурация из env ───────────────────────────────────────────────────
	kafkaBrokers := getEnv("KAFKA_BOOTSTRAP_SERVERS", "redpanda:9092")
	kafkaTopic := getEnv("KAFKA_TOPIC", "siem.events")
	kafkaGroup := getEnv("KAFKA_GROUP_ID", "detection-engine")
	redisAddr := getEnv("REDIS_ADDR", "redis:6379")
	redisPassword := getEnv("REDIS_PASSWORD", "")
	metricsAddr := getEnv("METRICS_ADDR", ":9110")

	logger.Info("starting detection engine",
		zap.String("kafka", kafkaBrokers),
		zap.String("topic", kafkaTopic),
		zap.String("redis", redisAddr),
	)

	// ── Redis state store ─────────────────────────────────────────────────────
	var stateStore rules.StateStore
	redisStore := correlator.NewRedisStore(redisAddr, redisPassword, 0)
	if err := redisStore.Ping(); err != nil {
		logger.Warn("Redis unavailable, running in stateless mode", zap.Error(err))
	} else {
		stateStore = redisStore
		defer redisStore.Close() //nolint:errcheck
		logger.Info("connected to Redis")
	}

	// ── Правила ───────────────────────────────────────────────────────────────
	sqliRule := rules.NewSQLInjectionRule()

	statelessRules := []rules.Rule{
		sqliRule,
	}

	bruteForceRule := rules.NewBruteForceRule()
	rateLimitRule := rules.NewRateLimitEvasionRule()
	privEscRule := rules.NewPrivilegeEscalationRule()

	statefulRules := []rules.StatefulRule{
		bruteForceRule,
		rateLimitRule,
		privEscRule,
	}

	// ── Alert channel ─────────────────────────────────────────────────────────
	alertCh := make(chan *rules.Alert, 1000)

	// ── Detection engine ──────────────────────────────────────────────────────
	eng := engine.New(statelessRules, statefulRules, stateStore, alertCh, logger)
	logger.Info("detection engine initialized", zap.Int("rules", eng.RuleCount()))

	// ── Alert sink (в реальной системе — публикует в Kafka/Alertmanager) ──────
	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	go func() {
		for {
			select {
			case alert := <-alertCh:
				logger.Warn("ALERT",
					zap.String("rule_id", alert.RuleID),
					zap.String("severity", string(alert.Severity)),
					zap.String("description", alert.Description),
					zap.Strings("mitre_tags", alert.MitreTags),
					zap.Time("fired_at", alert.FiredAt),
				)
			case <-ctx.Done():
				return
			}
		}
	}()

	// ── Kafka consumer ────────────────────────────────────────────────────────
	go runKafkaConsumer(ctx, kafkaBrokers, kafkaTopic, kafkaGroup, eng, logger)

	// ── Prometheus metrics ────────────────────────────────────────────────────
	mux := http.NewServeMux()
	mux.Handle("/metrics", promhttp.Handler())
	mux.HandleFunc("/health", func(w http.ResponseWriter, _ *http.Request) {
		w.Header().Set("Content-Type", "application/json")
		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"status":"ok","service":"detector"}`)) //nolint:errcheck
	})
	// /ready — проверяет реальные зависимости: Redis и Kafka broker.
	mux.HandleFunc("/ready", makeReadyHandler(kafkaBrokers, redisStore))

	srv := &http.Server{
		Addr:         metricsAddr,
		Handler:      mux,
		ReadTimeout:  5 * time.Second,
		WriteTimeout: 10 * time.Second,
	}
	go func() {
		logger.Info("metrics server starting", zap.String("addr", metricsAddr))
		if err := srv.ListenAndServe(); err != nil && err != http.ErrServerClosed {
			logger.Error("metrics server error", zap.Error(err))
		}
	}()

	// ── Graceful shutdown ─────────────────────────────────────────────────────
	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, syscall.SIGTERM, syscall.SIGINT)
	<-sigCh

	logger.Info("shutting down detection engine...")
	cancel()

	shutdownCtx, shutdownCancel := context.WithTimeout(context.Background(), 10*time.Second)
	defer shutdownCancel()
	srv.Shutdown(shutdownCtx) //nolint:errcheck

	logger.Info("shutdown complete")
}

func runKafkaConsumer(
	ctx context.Context,
	brokers, topic, group string,
	eng *engine.Engine,
	logger *zap.Logger,
) {
	brokerList := strings.Split(brokers, ",")

	reader := kafka.NewReader(kafka.ReaderConfig{
		Brokers:        brokerList,
		Topic:          topic,
		GroupID:        group,
		MinBytes:       1,
		MaxBytes:       10 << 20, // 10 MB
		MaxWait:        500 * time.Millisecond,
		CommitInterval: time.Second,
		// Начинаем с конца — обрабатываем только новые события после старта.
		StartOffset: kafka.LastOffset,
	})
	defer reader.Close() //nolint:errcheck

	logger.Info("Kafka consumer started",
		zap.String("brokers", brokers),
		zap.String("topic", topic),
		zap.String("group", group),
	)

	for {
		msg, err := reader.FetchMessage(ctx)
		if err != nil {
			if ctx.Err() != nil {
				logger.Info("Kafka consumer shutting down")
				return
			}
			logger.Warn("Kafka fetch error", zap.Error(err))
			time.Sleep(500 * time.Millisecond)
			continue
		}

		// Обновляем метрику лага из статистики reader.
		stats := reader.Stats()
		consumerLag.Set(float64(stats.Lag))

		// ProcessRaw: учёт JSON-ошибок и processed через engine metrics (без дубля с promauto).
		if len(msg.Value) == 0 {
			if err := reader.CommitMessages(ctx, msg); err != nil && ctx.Err() == nil {
				logger.Warn("commit failed on empty message", zap.Error(err))
			}
			continue
		}
		eng.ProcessRaw(msg.Value)

		if err := reader.CommitMessages(ctx, msg); err != nil && ctx.Err() == nil {
			logger.Warn("commit failed", zap.Error(err))
		}
	}
}

// makeReadyHandler возвращает HTTP handler для /ready, проверяющий Kafka и Redis.
func makeReadyHandler(kafkaBrokers string, store interface{ Ping() error }) http.HandlerFunc {
	return func(w http.ResponseWriter, r *http.Request) {
		w.Header().Set("Content-Type", "application/json")

		// Проверяем Redis
		if store != nil {
			if err := store.Ping(); err != nil {
				w.WriteHeader(http.StatusServiceUnavailable)
				w.Write([]byte(`{"status":"not_ready","reason":"redis_unavailable"}`)) //nolint:errcheck
				return
			}
		}

		// Проверяем достижимость Kafka брокера через dial
		checkCtx, cancel := context.WithTimeout(r.Context(), 3*time.Second)
		defer cancel()
		broker := strings.SplitN(kafkaBrokers, ",", 2)[0]
		conn, err := kafka.DialContext(checkCtx, "tcp", broker)
		if err != nil {
			w.WriteHeader(http.StatusServiceUnavailable)
			w.Write([]byte(`{"status":"not_ready","reason":"kafka_unavailable"}`)) //nolint:errcheck
			return
		}
		conn.Close() //nolint:errcheck

		w.WriteHeader(http.StatusOK)
		w.Write([]byte(`{"status":"ready"}`)) //nolint:errcheck
	}
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}

func getEnv(key, fallback string) string {
	if val := os.Getenv(key); val != "" {
		return val
	}
	return fallback
}

