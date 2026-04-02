package rules

import (
	"testing"
	"time"
)

func makeHTTPEvent(ip, sourceType string) *Event {
	return &Event{
		EventID:    "rle-test-id",
		SourceType: sourceType,
		EventType:  "application",
		Severity:   "info",
		Message:    "HTTP request",
		Host:       "api-01",
		Timestamp:  time.Now(),
		SourceIP:   strPtr(ip),
		Metadata:   map[string]any{},
	}
}

// ── RateLimitEvasionRule ─────────────────────────────────────────────────────

func TestRLE_AlertAtThreshold(t *testing.T) {
	rule := NewRateLimitEvasionRule()
	rule.Threshold = 10
	store := NewMockStateStore()
	event := makeHTTPEvent("3.3.3.3", "dotnet")

	store.SetCounter("rle:3.3.3.3", int64(rule.Threshold-1))

	alert := rule.Evaluate(event, store)
	if alert == nil {
		t.Fatal("Expected alert at threshold")
	}
	if alert.RuleID != "rate_limit_evasion" {
		t.Errorf("Unexpected rule ID: %s", alert.RuleID)
	}
	if alert.Severity != SeverityMedium {
		t.Errorf("Expected Medium severity, got %s", alert.Severity)
	}
}

func TestRLE_NoAlertBeforeThreshold(t *testing.T) {
	rule := NewRateLimitEvasionRule()
	rule.Threshold = 100
	store := NewMockStateStore()
	event := makeHTTPEvent("3.3.3.3", "dotnet")

	for i := 0; i < 50; i++ {
		alert := rule.Evaluate(event, store)
		if alert != nil {
			t.Fatalf("Unexpected alert at request %d", i+1)
		}
	}
}

func TestRLE_IgnoresUnknownSourceType(t *testing.T) {
	rule := NewRateLimitEvasionRule()
	rule.Threshold = 2
	store := NewMockStateStore()
	event := makeHTTPEvent("3.3.3.3", "postgresql")
	store.SetCounter("rle:3.3.3.3", int64(rule.Threshold-1))

	alert := rule.Evaluate(event, store)
	if alert != nil {
		t.Error("Should not alert for postgresql source type")
	}
}

func TestRLE_IgnoresKnownBot(t *testing.T) {
	rule := NewRateLimitEvasionRule()
	rule.Threshold = 2
	store := NewMockStateStore()
	event := makeHTTPEvent("3.3.3.3", "dotnet")
	event.Metadata["UserAgent"] = "Googlebot/2.1 (+http://www.google.com/bot.html)"
	store.SetCounter("rle:3.3.3.3", int64(rule.Threshold-1))

	alert := rule.Evaluate(event, store)
	if alert != nil {
		t.Error("Should not alert for known bot user agents")
	}
}

func TestRLE_IgnoresHealthCheck(t *testing.T) {
	rule := NewRateLimitEvasionRule()
	rule.Threshold = 2
	store := NewMockStateStore()
	event := makeHTTPEvent("3.3.3.3", "dotnet")
	event.Metadata["UserAgent"] = "health-check/1.0 uptime-monitor"
	store.SetCounter("rle:3.3.3.3", int64(rule.Threshold-1))

	alert := rule.Evaluate(event, store)
	if alert != nil {
		t.Error("Should not alert for health check user agents")
	}
}

func TestRLE_NoIP_NoAlert(t *testing.T) {
	rule := NewRateLimitEvasionRule()
	store := NewMockStateStore()
	event := makeHTTPEvent("3.3.3.3", "dotnet")
	event.SourceIP = nil

	alert := rule.Evaluate(event, store)
	if alert != nil {
		t.Error("Should not alert without source IP")
	}
}

func TestRLE_NginxSourceType(t *testing.T) {
	rule := NewRateLimitEvasionRule()
	rule.Threshold = 3
	store := NewMockStateStore()
	event := makeHTTPEvent("4.4.4.4", "nginx")
	store.SetCounter("rle:4.4.4.4", int64(rule.Threshold-1))

	alert := rule.Evaluate(event, store)
	if alert == nil {
		t.Fatal("Expected alert for nginx source type")
	}
}

func TestRLE_ContextContainsRequestCount(t *testing.T) {
	rule := NewRateLimitEvasionRule()
	rule.Threshold = 2
	store := NewMockStateStore()
	event := makeHTTPEvent("5.5.5.5", "dotnet")
	store.SetCounter("rle:5.5.5.5", int64(rule.Threshold-1))

	alert := rule.Evaluate(event, store)
	if alert == nil {
		t.Fatal("Expected alert")
	}
	if _, ok := alert.Context["request_count"]; !ok {
		t.Error("Expected request_count in context")
	}
}
