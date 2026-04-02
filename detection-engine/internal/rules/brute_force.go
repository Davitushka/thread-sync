package rules

import (
	"fmt"
	"strings"
	"time"
)

// BruteForceRule детектирует brute-force атаки на API / SignalR auth.
// Sigma: sigma-rules/brute_force_api.yaml
// MITRE: T1110, T1110.001 (Brute Force: Password Guessing)
//
// Логика: COUNT(failed_auth) >= threshold за window по одному source_ip.
// Состояние хранится в Redis, ключ: "bf:{source_ip}" TTL=window.
type BruteForceRule struct {
	Threshold int           // количество неудачных попыток
	Window    time.Duration // временное окно
}

func NewBruteForceRule() *BruteForceRule {
	return &BruteForceRule{
		Threshold: 10,
		Window:    2 * time.Minute,
	}
}

func (r *BruteForceRule) ID() string    { return "brute_force_api" }
func (r *BruteForceRule) Title() string { return "API / SignalR Brute-Force Authentication Attempts" }

// Match фильтрует события — возвращает событие-кандидат если оно подходит
// под паттерн brute-force (401/403 на auth endpoints).
// Сам алерт генерируется в Evaluate после накопления счётчика.
func (r *BruteForceRule) Match(event *Event) *Alert {
	// Фильтр 1: только HTTP события с кодом ошибки авторизации
	if event.StatusCode == nil {
		return nil
	}
	code := *event.StatusCode
	if code != 401 && code != 403 {
		return nil
	}

	// Фильтр 2: auth endpoints
	path := StrVal(event.URLPath)
	authPaths := []string{"/api/auth", "/api/login", "/api/token", "/hubs/", "/api/account"}
	isAuthPath := false
	for _, p := range authPaths {
		if strings.Contains(path, p) {
			isAuthPath = true
			break
		}
	}
	if !isAuthPath {
		return nil
	}

	// Фильтр 3: должен быть source_ip
	if event.SourceIP == nil {
		return nil
	}

	return nil // алерт генерируется в Evaluate
}

// Evaluate обновляет счётчик в Redis и возвращает алерт при превышении порога.
func (r *BruteForceRule) Evaluate(event *Event, state StateStore) *Alert {
	if event.SourceIP == nil {
		return nil
	}

	// Проверяем кандидата (повторяем фильтр из Match)
	if event.StatusCode == nil {
		return nil
	}
	code := *event.StatusCode
	if code != 401 && code != 403 {
		return nil
	}
	path := StrVal(event.URLPath)
	authPaths := []string{"/api/auth", "/api/login", "/api/token", "/hubs/", "/api/account"}
	isAuthPath := false
	for _, p := range authPaths {
		if strings.Contains(path, p) {
			isAuthPath = true
			break
		}
	}
	if !isAuthPath {
		return nil
	}

	key := fmt.Sprintf("bf:%s", *event.SourceIP)
	count, err := state.Increment(key, r.Window)
	if err != nil {
		return nil
	}

	// Алерт только при точном пересечении порога (чтобы не спамить)
	if count != int64(r.Threshold) {
		return nil
	}

	ip := *event.SourceIP
	return &Alert{
		RuleID:    r.ID(),
		RuleTitle: r.Title(),
		Severity:  SeverityHigh,
		Description: fmt.Sprintf(
			"Brute-force detected: %d failed authentication attempts in %v from %s",
			r.Threshold, r.Window, ip,
		),
		SourceIP:  &ip,
		UserID:    event.UserID,
		EventIDs:  []string{event.EventID},
		MitreTags: []string{"T1110", "T1110.001"},
		FiredAt:   time.Now().UTC(),
		Context: map[string]any{
			"failed_attempts": count,
			"window":         r.Window.String(),
			"url_path":       path,
		},
	}
}
