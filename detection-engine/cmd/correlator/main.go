// Correlator — stateful сервис корреляции событий SIEM-Lite.
//
// Отвечает за:
//   - Потребление событий из Kafka
//   - Применение stateful правил с Redis sliding window
//   - Отправку алертов в Alertmanager
//   - HTTP endpoint для health/metrics
//   - Background jobs: очистка устаревших счётчиков, статистика
package main

import (
	"bytes"
	"context"
	"encoding/json"
	"fmt"
	"net/http"
	"os"
	"os/signal"
	"strings"
	"sync"
	"syscall"
	"time"

	"github.com/prometheus/client_golang/prometheus"
	"github.com/prometheus/client_golang/prometheus/promauto"
	"github.com/prometheus/client_golang/prometheus/promhttp"
	kafka "github.com/segmentio/kafka-go"
	"go.uber.org/zap"
	"go.uber.org/zap/zapcore"

	"github.com/siem-lite/detection-engine/internal/correlator"
	"github.com/siem-lite/detection-engine/internal/engine"
	"github.com/siem-lite/detection-engine/internal/rules"
)

var (
	corrEventsProcessed = promauto.NewCounter(prometheus.CounterOpts{
		Name: "correlator_events_processed_total",
		Help: "Total events consumed from Kafka by correlator",
	})
	corrParseErrors = promauto.NewCounter(prometheus.CounterOpts{
		Name: "correlator_parse_errors_total",
		Help: "Total JSON parse errors in correlator consumer",
	})
	corrAlertsForwarded = promauto.NewCounter(prometheus.CounterOpts{
		Name: "correlator_alerts_forwarded_total",
		Help: "Total alerts forwarded to Alertmanager",
	})
)

// AlertmanagerAlert — структура для отправки алертов в Alertmanager API.
type AlertmanagerAlert struct {
	Labels      map[string]string `json:"labels"`
	Annotations map[string]string `json:"annotations"`
	StartsAt    time.Time         `json:"startsAt"`
	EndsAt      *time.Time        `json:"endsAt,omitempty"`
}

// Correlator — основная структура сервиса.
type Correlator struct {
	cfg    *Config
	logger *zap.Logger
	eng    *engine.Engine
	store  *correlator.RedisStore
	alertCh chan *rules.Alert
	wg     sync.WaitGroup
}

func main() {
	cfg, err := LoadFromEnv()
	if err != nil {
		fmt.Fprintf(os.Stderr, "Config error: %v\n", err)
		os.Exit(1)
	}
	if err := cfg.Validate(); err != nil {
		fmt.Fprintf(os.Stderr, "Invalid config: %v\n", err)
		os.Exit(1)
	}

	logger := buildLogger(cfg.LogLevel)
	defer logger.Sync() //nolint:errcheck

	logger.Info("starting correlator",
		zap.String("http_addr", cfg.HTTPAddr),
		zap.String("redis", cfg.RedisAddr),
		zap.String("kafka", cfg.KafkaBrokers),
		zap.String("alertmanager", cfg.AlertmanagerURL),
	)

	// ── Redis ─────────────────────────────────────────────────────────────────
	redisStore := correlator.NewRedisStore(cfg.RedisAddr, cfg.RedisPassword, cfg.RedisDB)
	if err := redisStore.Ping(); err != nil {
		logger.Warn("Redis unavailable at startup — stateful rules disabled",
			zap.String("addr", cfg.RedisAddr),
			zap.Error(err),
		)
	} else {
		logger.Info("Redis connected", zap.String("addr", cfg.RedisAddr))
	}
	defer redisStore.Close() //nolint:errcheck

	// ── Detection rules ───────────────────────────────────────────────────────
	alertCh := make(chan *rules.Alert, 1000)

	bruteForce := rules.NewBruteForceRule()
	bruteForce.Threshold = cfg.BruteForceThreshold
	bruteForce.Window = cfg.BruteForceWindow

	rateLimitRule := rules.NewRateLimitEvasionRule()
	rateLimitRule.Threshold = cfg.RateLimitThreshold
	rateLimitRule.Window = cfg.RateLimitWindow

	privEscRule := rules.NewPrivilegeEscalationRule()
	privEscRule.Threshold = cfg.PrivEscThreshold

	sqliRule := rules.NewSQLInjectionRule()

	statelessRules := []rules.Rule{sqliRule}
	statefulRules := []rules.StatefulRule{bruteForce, rateLimitRule, privEscRule}

	eng := engine.New(statelessRules, statefulRules, redisStore, alertCh, logger)
	logger.Info("detection engine ready", zap.Int("rules", eng.RuleCount()))

	// ── Correlator ────────────────────────────────────────────────────────────
	svc := &Correlator{
		cfg:     cfg,
		logger:  logger,
		eng:     eng,
		store:   redisStore,
		alertCh: alertCh,
	}

	ctx, cancel := context.WithCancel(context.Background())
	defer cancel()

	// Background jobs
	svc.wg.Add(1)
	go svc.runAlertForwarder(ctx)

	svc.wg.Add(1)
	go svc.runKafkaConsumer(ctx)

	svc.wg.Add(1)
	go svc.runStatsReporter(ctx)

	// ── HTTP server ───────────────────────────────────────────────────────────
	mux := http.NewServeMux()
	mux.Handle("/metrics", promhttp.Handler())
	mux.HandleFunc("/health", svc.handleHealth)
	mux.HandleFunc("/ready", svc.handleReady)
	mux.HandleFunc("/api/v1/stats", svc.handleStats)
	mux.HandleFunc("/api/v1/rules", svc.handleRules)

	srv := &http.Server{
		Addr:         cfg.HTTPAddr,
		Handler:      mux,
		ReadTimeout:  5 * time.Second,
		WriteTimeout: 10 * time.Second,
		IdleTimeout:  60 * time.Second,
	}

	go func() {
		logger.Info("HTTP server starting", zap.String("addr", cfg.HTTPAddr))
		if err := srv.ListenAndServe(); err != nil && err != http.ErrServerClosed {
			logger.Error("HTTP server error", zap.Error(err))
		}
	}()

	// ── Graceful shutdown ─────────────────────────────────────────────────────
	sigCh := make(chan os.Signal, 1)
	signal.Notify(sigCh, syscall.SIGTERM, syscall.SIGINT)
	sig := <-sigCh

	logger.Info("received shutdown signal", zap.String("signal", sig.String()))
	cancel()

	shutdownCtx, shutdownCancel := context.WithTimeout(context.Background(), cfg.ShutdownTimeout)
	defer shutdownCancel()

	if err := srv.Shutdown(shutdownCtx); err != nil {
		logger.Warn("HTTP server shutdown error", zap.Error(err))
	}

	// Ждём завершения background goroutines
	done := make(chan struct{})
	go func() {
		svc.wg.Wait()
		close(done)
	}()

	select {
	case <-done:
		logger.Info("all goroutines stopped")
	case <-shutdownCtx.Done():
		logger.Warn("shutdown timeout, forcing exit")
	}

	logger.Info("correlator stopped")
}

// ── Alert Forwarder ────────────────────────────────────────────────────────────

func (s *Correlator) runAlertForwarder(ctx context.Context) {
	defer s.wg.Done()
	client := &http.Client{Timeout: 10 * time.Second}

	for {
		select {
		case alert := <-s.alertCh:
			if err := s.forwardToAlertmanager(ctx, client, alert); err != nil {
				s.logger.Warn("failed to forward alert to Alertmanager",
					zap.String("rule_id", alert.RuleID),
					zap.Error(err),
				)
			} else {
				s.logger.Info("alert forwarded",
					zap.String("rule_id", alert.RuleID),
					zap.String("severity", string(alert.Severity)),
				)
			}
		case <-ctx.Done():
			return
		}
	}
}

func (s *Correlator) forwardToAlertmanager(ctx context.Context, client *http.Client, alert *rules.Alert) error {
	amAlert := AlertmanagerAlert{
		Labels: map[string]string{
			"alertname": alert.RuleTitle,
			"rule_id":   alert.RuleID,
			"severity":  string(alert.Severity),
		},
		Annotations: map[string]string{
			"description": alert.Description,
			"mitre_tags":  fmt.Sprintf("%v", alert.MitreTags),
		},
		StartsAt: alert.FiredAt,
	}

	if alert.SourceIP != nil {
		amAlert.Labels["source_ip"] = *alert.SourceIP
	}
	if alert.UserID != nil {
		amAlert.Labels["user_id"] = *alert.UserID
	}

	payload, err := json.Marshal([]AlertmanagerAlert{amAlert})
	if err != nil {
		return fmt.Errorf("marshal alert: %w", err)
	}

	url := s.cfg.AlertmanagerURL + "/api/v2/alerts"
	req, err := http.NewRequestWithContext(ctx, http.MethodPost, url, bytes.NewReader(payload))
	if err != nil {
		return fmt.Errorf("create request: %w", err)
	}
	req.Header.Set("Content-Type", "application/json")

	resp, err := client.Do(req)
	if err != nil {
		return fmt.Errorf("send to alertmanager: %w", err)
	}
	defer resp.Body.Close()

	if resp.StatusCode >= 300 {
		return fmt.Errorf("alertmanager responded %d", resp.StatusCode)
	}
	return nil
}

// ── Kafka Consumer ─────────────────────────────────────────────────────────────

func (s *Correlator) runKafkaConsumer(ctx context.Context) {
	defer s.wg.Done()

	brokerList := strings.Split(s.cfg.KafkaBrokers, ",")
	reader := kafka.NewReader(kafka.ReaderConfig{
		Brokers:        brokerList,
		Topic:          s.cfg.KafkaTopic,
		GroupID:        s.cfg.KafkaGroupID,
		MinBytes:       1,
		MaxBytes:       10 << 20,
		MaxWait:        500 * time.Millisecond,
		CommitInterval: time.Second,
		StartOffset:    kafka.LastOffset,
	})
	defer reader.Close() //nolint:errcheck

	s.logger.Info("Kafka consumer started",
		zap.String("brokers", s.cfg.KafkaBrokers),
		zap.String("topic", s.cfg.KafkaTopic),
		zap.String("group", s.cfg.KafkaGroupID),
	)

	for {
		msg, err := reader.FetchMessage(ctx)
		if err != nil {
			if ctx.Err() != nil {
				s.logger.Info("Kafka consumer shutting down")
				return
			}
			s.logger.Warn("Kafka fetch error", zap.Error(err))
			time.Sleep(500 * time.Millisecond)
			continue
		}

		var event rules.Event
		if err := json.Unmarshal(msg.Value, &event); err != nil {
			corrParseErrors.Inc()
			s.logger.Warn("JSON parse error",
				zap.Error(err),
				zap.ByteString("raw", msg.Value[:minBytes(len(msg.Value), 200)]),
			)
			if err := reader.CommitMessages(ctx, msg); err != nil && ctx.Err() == nil {
				s.logger.Warn("commit failed after parse error", zap.Error(err))
			}
			continue
		}

		s.eng.Process(&event)
		corrEventsProcessed.Inc()

		if err := reader.CommitMessages(ctx, msg); err != nil && ctx.Err() == nil {
			s.logger.Warn("commit failed", zap.Error(err))
		}
	}
}

func minBytes(a, b int) int {
	if a < b {
		return a
	}
	return b
}

// ── Stats Reporter ─────────────────────────────────────────────────────────────

func (s *Correlator) runStatsReporter(ctx context.Context) {
	defer s.wg.Done()
	ticker := time.NewTicker(60 * time.Second)
	defer ticker.Stop()

	for {
		select {
		case <-ticker.C:
			s.logger.Info("correlator stats",
				zap.Int("active_rules", s.eng.RuleCount()),
				zap.Int("pending_alerts", len(s.alertCh)),
			)
		case <-ctx.Done():
			return
		}
	}
}

// ── HTTP Handlers ──────────────────────────────────────────────────────────────

func (s *Correlator) handleHealth(w http.ResponseWriter, _ *http.Request) {
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(http.StatusOK)
	fmt.Fprint(w, `{"status":"ok","service":"correlator"}`)
}

func (s *Correlator) handleReady(w http.ResponseWriter, r *http.Request) {
	w.Header().Set("Content-Type", "application/json")

	if err := s.store.Ping(); err != nil {
		w.WriteHeader(http.StatusServiceUnavailable)
		fmt.Fprint(w, `{"status":"not_ready","reason":"redis_unavailable"}`)
		return
	}

	// Быстрая проверка доступности Kafka (TCP dial).
	checkCtx, cancel := context.WithTimeout(r.Context(), 3*time.Second)
	defer cancel()
	broker := strings.SplitN(s.cfg.KafkaBrokers, ",", 2)[0]
	conn, err := kafka.DialContext(checkCtx, "tcp", broker)
	if err != nil {
		w.WriteHeader(http.StatusServiceUnavailable)
		fmt.Fprint(w, `{"status":"not_ready","reason":"kafka_unavailable"}`)
		return
	}
	conn.Close() //nolint:errcheck

	w.WriteHeader(http.StatusOK)
	fmt.Fprint(w, `{"status":"ready","service":"correlator"}`)
}

func (s *Correlator) handleStats(w http.ResponseWriter, _ *http.Request) {
	stats := map[string]interface{}{
		"rules_count":    s.eng.RuleCount(),
		"pending_alerts": len(s.alertCh),
		"alert_capacity": cap(s.alertCh),
		"timestamp":      time.Now().UTC().Format(time.RFC3339),
	}
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(stats) //nolint:errcheck
}

func (s *Correlator) handleRules(w http.ResponseWriter, _ *http.Request) {
	ruleInfo := []map[string]interface{}{
		{"id": "brute_force_api", "type": "stateful", "threshold": s.cfg.BruteForceThreshold, "window": s.cfg.BruteForceWindow.String()},
		{"id": "rate_limit_evasion", "type": "stateful", "threshold": s.cfg.RateLimitThreshold, "window": s.cfg.RateLimitWindow.String()},
		{"id": "privilege_escalation_attempt", "type": "stateful", "threshold": s.cfg.PrivEscThreshold, "window": s.cfg.PrivEscWindow.String()},
		{"id": "sql_injection_attempt", "type": "stateless"},
	}
	w.Header().Set("Content-Type", "application/json")
	json.NewEncoder(w).Encode(ruleInfo) //nolint:errcheck
}

// ── Logger builder ────────────────────────────────────────────────────────────

func buildLogger(level string) *zap.Logger {
	var zapLevel zapcore.Level
	switch level {
	case "debug":
		zapLevel = zapcore.DebugLevel
	case "warn", "warning":
		zapLevel = zapcore.WarnLevel
	case "error":
		zapLevel = zapcore.ErrorLevel
	default:
		zapLevel = zapcore.InfoLevel
	}

	cfg := zap.NewProductionConfig()
	cfg.Level = zap.NewAtomicLevelAt(zapLevel)
	logger, err := cfg.Build()
	if err != nil {
		panic("failed to build logger: " + err.Error())
	}
	return logger
}
