package rules

import (
	"fmt"
	"strings"
	"time"
)

// RateLimitEvasionRule детектирует аномально высокий объём запросов с одного IP.
// Sigma: sigma-rules/rate_limit_evasion.yaml
// MITRE: T1595 (Active Scanning), T1046 (Network Service Discovery)
//
// Логика: COUNT(requests) >= threshold за window по source_ip.
// Исключаем известных ботов по User-Agent.
type RateLimitEvasionRule struct {
	Threshold int           // max запросов за window
	Window    time.Duration // временное окно
}

func NewRateLimitEvasionRule() *RateLimitEvasionRule {
	return &RateLimitEvasionRule{
		Threshold: 500,
		Window:    time.Minute,
	}
}

func (r *RateLimitEvasionRule) ID() string {
	return "rate_limit_evasion"
}
func (r *RateLimitEvasionRule) Title() string {
	return "Rate Limit Evasion — Anomalous Request Volume from Single IP"
}

// knownBots содержит User-Agent подстроки легитимных краулеров.
var knownBots = []string{
	"googlebot", "bingbot", "health-check", "uptime-robot", "pingdom", "datadog",
}

func isKnownBot(userAgent string) bool {
	ua := strings.ToLower(userAgent)
	for _, bot := range knownBots {
		if strings.Contains(ua, bot) {
			return true
		}
	}
	return false
}

func (r *RateLimitEvasionRule) Match(event *Event) *Alert {
	return nil // stateful — алерт только в Evaluate
}

func (r *RateLimitEvasionRule) Evaluate(event *Event, state StateStore) *Alert {
	if event.SourceIP == nil {
		return nil
	}

	// Фильтр: application события
	if event.SourceType != "dotnet" && event.SourceType != "nginx" {
		return nil
	}

	// Фильтр: исключаем известных ботов
	if ua, ok := event.Metadata["UserAgent"].(string); ok && isKnownBot(ua) {
		return nil
	}

	key := fmt.Sprintf("rle:%s", *event.SourceIP)
	count, err := state.Increment(key, r.Window)
	if err != nil {
		return nil
	}

	// Алерт при пересечении порога
	if count != int64(r.Threshold) {
		return nil
	}

	ip := *event.SourceIP
	return &Alert{
		RuleID:    r.ID(),
		RuleTitle: r.Title(),
		Severity:  SeverityMedium,
		Description: fmt.Sprintf(
			"High request volume: %d requests in %v from %s (possible rate limit bypass)",
			r.Threshold, r.Window, ip,
		),
		SourceIP:  &ip,
		EventIDs:  []string{event.EventID},
		MitreTags: []string{"T1595", "T1595.002", "T1046"},
		FiredAt:   time.Now().UTC(),
		Context: map[string]any{
			"request_count": count,
			"window":        r.Window.String(),
			"threshold":     r.Threshold,
		},
	}
}
