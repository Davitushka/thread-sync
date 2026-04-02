package rules

import (
	"testing"
	"time"
)

func makeAdminEvent(ip, path, method string, code int, role string) *Event {
	meta := map[string]any{}
	if role != "" {
		meta["UserRole"] = role
	}
	return &Event{
		EventID:    "priv-test-id",
		SourceType: "dotnet",
		EventType:  "application",
		Severity:   "warning",
		Message:    "Admin endpoint access",
		Host:       "api-01",
		Timestamp:  time.Now(),
		SourceIP:   strPtr(ip),
		StatusCode: intPtr(code),
		URLPath:    strPtr(path),
		HTTPMethod: strPtr(method),
		Metadata:   meta,
	}
}

// ── PrivilegeEscalationRule ───────────────────────────────────────────────────

func TestPrivEsc_Forbidden403OnAdminPath(t *testing.T) {
	rule := NewPrivilegeEscalationRule()
	event := makeAdminEvent("10.0.0.1", "/api/admin/users", "GET", 403, "user")
	alert := rule.Match(event)
	if alert == nil {
		t.Fatal("Expected alert for 403 on admin path")
	}
	if alert.Severity != SeverityHigh {
		t.Errorf("Expected High severity, got %s", alert.Severity)
	}
}

func TestPrivEsc_NonAdminRoleOnAdminPath(t *testing.T) {
	rule := NewPrivilegeEscalationRule()
	event := makeAdminEvent("10.0.0.1", "/api/admin/config", "GET", 200, "viewer")
	alert := rule.Match(event)
	if alert == nil {
		t.Fatal("Expected alert for non-admin role accessing admin path")
	}
	if alert.Severity != SeverityCritical {
		t.Errorf("Expected Critical severity, got %s", alert.Severity)
	}
}

func TestPrivEsc_AdminRoleOnAdminPath_NoAlert(t *testing.T) {
	rule := NewPrivilegeEscalationRule()
	event := makeAdminEvent("10.0.0.1", "/api/admin/config", "GET", 200, "admin")
	alert := rule.Match(event)
	if alert != nil {
		t.Error("Expected no alert for legitimate admin access")
	}
}

func TestPrivEsc_RoleModification(t *testing.T) {
	rule := NewPrivilegeEscalationRule()
	event := makeAdminEvent("10.0.0.1", "/api/users/roles", "PUT", 200, "user")
	alert := rule.Match(event)
	if alert == nil {
		t.Fatal("Expected alert for role modification attempt")
	}
	if alert.Severity != SeverityCritical {
		t.Errorf("Expected Critical severity, got %s", alert.Severity)
	}
}

func TestPrivEsc_NormalPath_NoAlert(t *testing.T) {
	rule := NewPrivilegeEscalationRule()
	event := makeAdminEvent("10.0.0.1", "/api/products", "GET", 200, "user")
	alert := rule.Match(event)
	if alert != nil {
		t.Error("Expected no alert for normal path")
	}
}

func TestPrivEsc_Stateful_AlertAtThreshold(t *testing.T) {
	rule := NewPrivilegeEscalationRule()
	rule.Threshold = 3
	store := NewMockStateStore()
	event := makeAdminEvent("2.2.2.2", "/api/admin/users", "GET", 403, "")

	var alert *Alert
	for i := 0; i < 3; i++ {
		alert = rule.Evaluate(event, store)
	}
	if alert == nil {
		t.Fatal("Expected stateful alert at threshold")
	}
	if alert.Severity != SeverityCritical {
		t.Errorf("Expected Critical for repeated attempts, got %s", alert.Severity)
	}
}

func TestPrivEsc_Stateful_NoAlertBeforeThreshold(t *testing.T) {
	rule := NewPrivilegeEscalationRule()
	rule.Threshold = 5
	store := NewMockStateStore()
	event := makeAdminEvent("2.2.2.2", "/api/admin/users", "GET", 403, "")

	for i := 0; i < 4; i++ {
		alert := rule.Evaluate(event, store)
		if alert != nil {
			t.Fatalf("Unexpected alert at attempt %d", i+1)
		}
	}
}

func TestPrivEsc_ManagementPath(t *testing.T) {
	rule := NewPrivilegeEscalationRule()
	event := makeAdminEvent("10.0.0.1", "/api/management/stats", "GET", 403, "")
	alert := rule.Match(event)
	if alert == nil {
		t.Fatal("Expected alert for /api/management path")
	}
}

func TestPrivEsc_ActuatorPath(t *testing.T) {
	rule := NewPrivilegeEscalationRule()
	event := makeAdminEvent("10.0.0.1", "/actuator/env", "GET", 403, "")
	alert := rule.Match(event)
	if alert == nil {
		t.Fatal("Expected alert for /actuator path")
	}
}

func TestPrivEsc_MitreTags(t *testing.T) {
	rule := NewPrivilegeEscalationRule()
	event := makeAdminEvent("10.0.0.1", "/api/admin", "GET", 403, "")
	alert := rule.Match(event)
	if alert == nil {
		t.Fatal("Expected alert")
	}
	hasT1068 := false
	for _, tag := range alert.MitreTags {
		if tag == "T1068" {
			hasT1068 = true
		}
	}
	if !hasT1068 {
		t.Errorf("Expected T1068 in MITRE tags, got %v", alert.MitreTags)
	}
}
