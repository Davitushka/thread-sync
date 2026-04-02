// Команда детектора — запускает Kafka consumer + detection engine + HTTP metrics.
package main

import (
	"context"
	"net/http"
	"os"
	"os/signal"
	"syscall"
	"time"

	"github.com/prometheus/client_golang/prometheus/promhttp"
	"go.uber.org/zap"

	"github.com/siem-lite/detection-engine/internal/correlator"
	"github.com/siem-lite/detection-engine/internal/engine"
	"github.com/siem-lite/detection-engine/internal/rules"
)

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
		w.WriteHeader(http.StatusOK)
		w.Write([]byte("ok")) //nolint:errcheck
	})
	mux.HandleFunc("/ready", func(w http.ResponseWriter, _ *http.Request) {
		w.WriteHeader(http.StatusOK)
		w.Write([]byte("ready")) //nolint:errcheck
	})

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
	// Kafka consumer на librdkafka через confluent-kafka-go
	// (реализован как заглушка — полная реализация требует CGO)
	logger.Info("Kafka consumer stub — waiting for events",
		zap.String("brokers", brokers),
		zap.String("topic", topic),
		zap.String("group", group),
	)
	<-ctx.Done()
}

func getEnv(key, fallback string) string {
	if val := os.Getenv(key); val != "" {
		return val
	}
	return fallback
}
