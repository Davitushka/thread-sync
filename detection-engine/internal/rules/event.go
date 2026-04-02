// Package rules содержит типы событий и структуры правил детектирования.
package rules

import "time"

// Event — нормализованное событие из Kafka (соответствует NormalizedEvent из Rust).
type Event struct {
	Timestamp  time.Time         `json:"@timestamp"`
	EventID    string            `json:"event_id"`
	SourceType string            `json:"source_type"`
	EventType  string            `json:"event_type"`
	Severity   string            `json:"severity"`
	Message    string            `json:"message"`
	Host       string            `json:"host"`
	SourceIP   *string           `json:"source_ip"`
	UserID     *string           `json:"user_id"`
	Action     *string           `json:"action"`
	StatusCode *int              `json:"status_code"`
	URLPath    *string           `json:"url_path"`
	HTTPMethod *string           `json:"http_method"`
	DurationMs *float64          `json:"duration_ms"`
	Metadata   map[string]any    `json:"metadata"`
}

// StrVal возвращает строковое значение указателя или пустую строку.
func StrVal(s *string) string {
	if s == nil {
		return ""
	}
	return *s
}

// IntVal возвращает int значение указателя или 0.
func IntVal(i *int) int {
	if i == nil {
		return 0
	}
	return *i
}
