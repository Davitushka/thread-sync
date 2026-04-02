// Package rules содержит типы и интерфейс детектирующих правил.
package rules

import "time"

// Severity уровни алертов.
type AlertSeverity string

const (
	SeverityLow      AlertSeverity = "low"
	SeverityMedium   AlertSeverity = "medium"
	SeverityHigh     AlertSeverity = "high"
	SeverityCritical AlertSeverity = "critical"
)

// Alert генерируется правилом при срабатывании.
type Alert struct {
	RuleID      string        `json:"rule_id"`
	RuleTitle   string        `json:"rule_title"`
	Severity    AlertSeverity `json:"severity"`
	Description string        `json:"description"`
	SourceIP    *string       `json:"source_ip,omitempty"`
	UserID      *string       `json:"user_id,omitempty"`
	EventIDs    []string      `json:"event_ids"`
	MitreTags   []string      `json:"mitre_tags"`
	FiredAt     time.Time     `json:"fired_at"`
	// Дополнительный контекст из события
	Context map[string]any `json:"context,omitempty"`
}

// Rule — интерфейс детектирующего правила.
// Каждое правило реализует Match для одиночного события
// и (опционально) Correlate для агрегированных состояний.
type Rule interface {
	// ID возвращает уникальный идентификатор правила.
	ID() string
	// Title возвращает человекочитаемое название.
	Title() string
	// Match проверяет одиночное событие. Возвращает алерт или nil.
	Match(event *Event) *Alert
}

// StatefulRule — правило с состоянием (sliding window, rate counters).
// Реализуется отдельно от Rule т.к. требует доступа к Redis.
type StatefulRule interface {
	Rule
	// Evaluate вызывается после каждого Match для обновления состояния.
	// Возвращает алерт если накопленное состояние превысило порог.
	Evaluate(event *Event, state StateStore) *Alert
}

// StateStore — интерфейс хранилища состояния (Redis).
type StateStore interface {
	// Increment атомарно увеличивает счётчик и возвращает новое значение.
	Increment(key string, ttl time.Duration) (int64, error)
	// Get возвращает текущее значение счётчика.
	Get(key string) (int64, error)
	// AddToSet добавляет элемент в множество и возвращает его размер.
	AddToSet(key string, member string, ttl time.Duration) (int64, error)
	// SetSize возвращает количество элементов в множестве.
	SetSize(key string) (int64, error)
}
