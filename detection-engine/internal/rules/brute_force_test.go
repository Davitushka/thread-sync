package rules

import (
	"testing"
	"time"
)

func intPtr(n int) *int    { return &n }
func strPtr(s string) *string { return &s }

func makeAuthEvent(ip string, code int, path string) *Event {
	return &Event{
		EventID:    "test-event-id",
		SourceType: "dotnet",
		EventType:  "application",
		Severity:   "warning",
		Message:    "Login failed",
		Host:       "api-01",
		Timestamp:  time.Now(),
		SourceIP:   strPtr(ip),
		StatusCode: intPtr(code),
		URLPath:    strPtr(path),
		Metadata:   map[string]any{},
	}
}

// ── BruteForceRule ────────────────────────────────────────────────────────────

func TestBruteForce_NoAlertBeforeThreshold(t *testing.T) {
	rule := NewBruteForceRule()
	rule.Threshold = 5
	store := NewMockStateStore()
	event := makeAuthEvent("1.2.3.4", 401, "/api/auth/login")

	for i := 0; i < 4; i++ {
		alert := rule.Evaluate(event, store)
		if alert != nil {
			t.Fatalf("Expected no alert before threshold, got alert at attempt %d", i+1)
		}
	}
}

func TestBruteForce_AlertAtThreshold(t *testing.T) {
	rule := NewBruteForceRule()
	rule.Threshold = 5
	store := NewMockStateStore()
	event := makeAuthEvent("1.2.3.4", 401, "/api/auth/login")

	var alert *Alert
	for i := 0; i < 5; i++ {
		alert = rule.Evaluate(event, store)
	}

	if alert == nil {
		t.Fatal("Expected alert at threshold, got nil")
	}
	if alert.Severity != SeverityHigh {
		t.Errorf("Expected High severity, got %s", alert.Severity)
	}
	if alert.RuleID != "brute_force_api" {
		t.Errorf("Unexpected rule ID: %s", alert.RuleID)
	}
	if alert.SourceIP == nil || *alert.SourceIP != "1.2.3.4" {
		t.Errorf("Expected source IP 1.2.3.4, got %v", alert.SourceIP)
	}
}

func TestBruteForce_NoAlertAfterThreshold(t *testing.T) {
	rule := NewBruteForceRule()
	rule.Threshold = 3
	store := NewMockStateStore()
	// Устанавливаем счётчик на пороге
	store.SetCounter("bf:1.2.3.4", 3)
	event := makeAuthEvent("1.2.3.4", 401, "/api/auth/login")

	// 4-й вызов — счётчик становится 4, не равен threshold=3
	alert := rule.Evaluate(event, store)
	if alert != nil {
		t.Error("Expected no duplicate alert after threshold, got alert")
	}
}

func TestBruteForce_IgnoresNon401(t *testing.T) {
	rule := NewBruteForceRule()
	store := NewMockStateStore()
	store.SetCounter("bf:1.2.3.4", int64(rule.Threshold-1))

	event := makeAuthEvent("1.2.3.4", 200, "/api/auth/login")
	alert := rule.Evaluate(event, store)
	if alert != nil {
		t.Error("Should not alert for HTTP 200")
	}
}

func TestBruteForce_IgnoresNonAuthPath(t *testing.T) {
	rule := NewBruteForceRule()
	rule.Threshold = 2
	store := NewMockStateStore()

	event := makeAuthEvent("1.2.3.4", 401, "/api/products")
	store.SetCounter("bf:1.2.3.4", int64(rule.Threshold-1))
	alert := rule.Evaluate(event, store)
	if alert != nil {
		t.Error("Should not alert for non-auth path")
	}
}

func TestBruteForce_IgnoresNoIP(t *testing.T) {
	rule := NewBruteForceRule()
	store := NewMockStateStore()
	event := makeAuthEvent("1.2.3.4", 401, "/api/auth/login")
	event.SourceIP = nil
	alert := rule.Evaluate(event, store)
	if alert != nil {
		t.Error("Should not alert for event without source IP")
	}
}

func TestBruteForce_SignalRPath(t *testing.T) {
	rule := NewBruteForceRule()
	rule.Threshold = 3
	store := NewMockStateStore()
	event := makeAuthEvent("5.5.5.5", 401, "/hubs/notifications")
	store.SetCounter("bf:5.5.5.5", int64(rule.Threshold-1))

	alert := rule.Evaluate(event, store)
	if alert == nil {
		t.Error("Expected alert for SignalR hub brute force")
	}
}

func TestBruteForce_MitreTags(t *testing.T) {
	rule := NewBruteForceRule()
	rule.Threshold = 1
	store := NewMockStateStore()
	event := makeAuthEvent("1.2.3.4", 401, "/api/login")

	alert := rule.Evaluate(event, store)
	if alert == nil {
		t.Fatal("Expected alert")
	}
	found := false
	for _, tag := range alert.MitreTags {
		if tag == "T1110" {
			found = true
		}
	}
	if !found {
		t.Errorf("Expected T1110 in MITRE tags, got %v", alert.MitreTags)
	}
}
