package rules

import (
	"testing"
	"time"
)

func makeSQLEvent(msg, path, sourceType string, code int) *Event {
	return &Event{
		EventID:    "sql-test-id",
		SourceType: sourceType,
		EventType:  "application",
		Severity:   "error",
		Message:    msg,
		Host:       "db-01",
		Timestamp:  time.Now(),
		SourceIP:   strPtr("10.0.0.1"),
		StatusCode: intPtr(code),
		URLPath:    strPtr(path),
		Metadata:   map[string]any{},
	}
}

// ── SQLInjectionRule ─────────────────────────────────────────────────────────

func TestSQLI_UnionSelect(t *testing.T) {
	rule := NewSQLInjectionRule()
	event := makeSQLEvent("SQL error: UNION SELECT username, password FROM users", "/api/search", "dotnet", 500)
	alert := rule.Match(event)
	if alert == nil {
		t.Fatal("Expected alert for UNION SELECT")
	}
	if alert.Severity != SeverityCritical {
		t.Errorf("Expected Critical for 500 status, got %s", alert.Severity)
	}
}

func TestSQLI_DropTable(t *testing.T) {
	rule := NewSQLInjectionRule()
	event := makeSQLEvent("Error executing: ; DROP TABLE users;--", "/api/data", "dotnet", 500)
	alert := rule.Match(event)
	if alert == nil {
		t.Fatal("Expected alert for DROP TABLE")
	}
}

func TestSQLI_OrTautology(t *testing.T) {
	rule := NewSQLInjectionRule()
	event := makeSQLEvent("Query failed: ' OR '1'='1", "/api/login", "dotnet", 400)
	alert := rule.Match(event)
	if alert == nil {
		t.Fatal("Expected alert for OR tautology")
	}
	if alert.Severity != SeverityHigh {
		t.Errorf("Expected High for 400 status, got %s", alert.Severity)
	}
}

func TestSQLI_InformationSchema(t *testing.T) {
	rule := NewSQLInjectionRule()
	event := makeSQLEvent("Attempt to access information_schema.tables detected", "/api/query", "postgresql", 403)
	alert := rule.Match(event)
	if alert == nil {
		t.Fatal("Expected alert for information_schema access")
	}
}

func TestSQLI_NoSQLMongoWhere(t *testing.T) {
	rule := NewSQLInjectionRule()
	event := makeSQLEvent(`Query: {"$where": "this.password.length > 0"}`, "/api/users", "dotnet", 200)
	alert := rule.Match(event)
	if alert == nil {
		t.Fatal("Expected alert for MongoDB $where injection")
	}
}

func TestSQLI_NoSQLRegex(t *testing.T) {
	rule := NewSQLInjectionRule()
	event := makeSQLEvent(`{"username": {"$regex": ".*"}}`, "/api/login", "dotnet", 200)
	alert := rule.Match(event)
	if alert == nil {
		t.Fatal("Expected alert for MongoDB $regex injection")
	}
}

func TestSQLI_CleanEvent_NoAlert(t *testing.T) {
	rule := NewSQLInjectionRule()
	event := makeSQLEvent("User logged in successfully", "/api/auth/login", "dotnet", 200)
	alert := rule.Match(event)
	if alert != nil {
		t.Error("Expected no alert for clean event")
	}
}

func TestSQLI_IgnoresUnknownSourceType(t *testing.T) {
	rule := NewSQLInjectionRule()
	event := makeSQLEvent("UNION SELECT id FROM users", "/api/data", "redis", 200)
	alert := rule.Match(event)
	if alert != nil {
		t.Error("Should not alert for redis source type (not in scope)")
	}
}

func TestSQLI_FalsePositive_HealthCheck(t *testing.T) {
	rule := NewSQLInjectionRule()
	// health check endpoint с SQL в названии параметра
	event := makeSQLEvent("health check passed", "/health-check", "dotnet", 200)
	alert := rule.Match(event)
	if alert != nil {
		t.Error("Should not alert for health check endpoints")
	}
}

func TestSQLI_HexEncoding(t *testing.T) {
	rule := NewSQLInjectionRule()
	event := makeSQLEvent("Error: 0x41414141 hex encoded payload detected", "/api/exec", "dotnet", 500)
	alert := rule.Match(event)
	if alert == nil {
		t.Fatal("Expected alert for hex encoding in payload")
	}
}

func TestSQLI_ContextContainsPatterns(t *testing.T) {
	rule := NewSQLInjectionRule()
	event := makeSQLEvent("UNION ALL SELECT null,null,null--", "/api/search", "dotnet", 400)
	alert := rule.Match(event)
	if alert == nil {
		t.Fatal("Expected alert")
	}
	if _, ok := alert.Context["matched_patterns"]; !ok {
		t.Error("Expected matched_patterns in context")
	}
}
