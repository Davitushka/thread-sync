// Package engine содержит основной детектирующий движок.
package engine

import (
	"encoding/json"
	"sync"
	"time"

	"go.uber.org/zap"

	"github.com/siem-lite/detection-engine/internal/rules"
)

// Engine — движок корреляции и детектирования.
// Потокобезопасен, применяет все зарегистрированные правила к входящему событию.
type Engine struct {
	statelessRules []rules.Rule
	statefulRules  []rules.StatefulRule
	state          rules.StateStore
	alertCh        chan<- *rules.Alert
	mu             sync.RWMutex
	logger         *zap.Logger
	metrics        *EngineMetrics
}

// Config — конфигурация движка.
type Config struct {
	AlertChannelBuffer int
	// DrainTimeout — сколько ждать опустошения alertCh при shutdown
	DrainTimeout time.Duration
}

// DefaultConfig возвращает конфигурацию с разумными дефолтами.
func DefaultConfig() Config {
	return Config{
		AlertChannelBuffer: 1000,
		DrainTimeout:       10 * time.Second,
	}
}

// New создаёт Engine с переданными правилами и state store.
func New(
	stateless []rules.Rule,
	stateful []rules.StatefulRule,
	state rules.StateStore,
	alertCh chan<- *rules.Alert,
	logger *zap.Logger,
) *Engine {
	return &Engine{
		statelessRules: stateless,
		statefulRules:  stateful,
		state:          state,
		alertCh:        alertCh,
		logger:         logger,
		metrics:        newEngineMetrics(),
	}
}

// ProcessRaw десериализует JSON события и запускает все правила.
// Вызывается из Kafka consumer goroutine.
func (e *Engine) ProcessRaw(payload []byte) {
	var event rules.Event
	if err := json.Unmarshal(payload, &event); err != nil {
		e.logger.Warn("failed to deserialize event", zap.Error(err), zap.ByteString("payload", payload[:min(len(payload), 200)]))
		e.metrics.parseErrors.Inc()
		return
	}
	e.Process(&event)
}

// Process применяет все правила к событию.
func (e *Engine) Process(event *rules.Event) {
	start := time.Now()
	e.metrics.eventsProcessed.Inc()

	e.mu.RLock()
	stateless := e.statelessRules
	stateful := e.statefulRules
	e.mu.RUnlock()

	// Stateless правила — параллельно безопасны, нет состояния
	for _, rule := range stateless {
		if alert := rule.Match(event); alert != nil {
			e.emitAlert(alert, rule.ID())
		}
	}

	// Stateful правила — Match + Evaluate
	for _, rule := range stateful {
		if e.state != nil {
			if alert := rule.Evaluate(event, e.state); alert != nil {
				e.emitAlert(alert, rule.ID())
			}
		} else {
			// Fallback: только stateless часть без Redis
			if alert := rule.Match(event); alert != nil {
				e.emitAlert(alert, rule.ID())
			}
		}
	}

	e.metrics.processDuration.Observe(time.Since(start).Seconds())
}

// AddRule добавляет правило в runtime (потокобезопасно).
func (e *Engine) AddRule(rule rules.Rule) {
	e.mu.Lock()
	defer e.mu.Unlock()
	e.statelessRules = append(e.statelessRules, rule)
}

// RuleCount возвращает суммарное количество правил.
func (e *Engine) RuleCount() int {
	e.mu.RLock()
	defer e.mu.RUnlock()
	return len(e.statelessRules) + len(e.statefulRules)
}

func (e *Engine) emitAlert(alert *rules.Alert, ruleID string) {
	e.metrics.alertsFired.WithLabelValues(string(alert.Severity), ruleID).Inc()
	e.logger.Info("alert fired",
		zap.String("rule_id", alert.RuleID),
		zap.String("severity", string(alert.Severity)),
		zap.String("description", alert.Description),
	)

	select {
	case e.alertCh <- alert:
	default:
		// Канал переполнен — логируем, не блокируем основной поток
		e.logger.Warn("alert channel full, dropping alert",
			zap.String("rule_id", ruleID),
			zap.String("severity", string(alert.Severity)),
		)
		e.metrics.alertsDropped.Inc()
	}
}

func min(a, b int) int {
	if a < b {
		return a
	}
	return b
}
