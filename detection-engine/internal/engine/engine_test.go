package engine

import (
	"encoding/json"
	"testing"
	"time"

	"go.uber.org/zap"

	"github.com/siem-lite/detection-engine/internal/rules"
)

// mockRule для тестирования движка.
type mockRule struct {
	id     string
	matchFn func(*rules.Event) *rules.Alert
}

func (r *mockRule) ID() string    { return r.id }
func (r *mockRule) Title() string { return "Mock Rule " + r.id }
func (r *mockRule) Match(e *rules.Event) *rules.Alert {
	if r.matchFn != nil {
		return r.matchFn(e)
	}
	return nil
}

func newTestEngine(stateless []rules.Rule) (*Engine, chan *rules.Alert) {
	alertCh := make(chan *rules.Alert, 100)
	logger := zap.NewNop()
	eng := New(stateless, nil, nil, alertCh, logger)
	return eng, alertCh
}

func TestEngine_RuleCount(t *testing.T) {
	rule1 := &mockRule{id: "r1"}
	rule2 := &mockRule{id: "r2"}
	eng, _ := newTestEngine([]rules.Rule{rule1, rule2})

	if count := eng.RuleCount(); count != 2 {
		t.Errorf("Expected 2 rules, got %d", count)
	}
}

func TestEngine_AddRule(t *testing.T) {
	eng, _ := newTestEngine(nil)
	eng.AddRule(&mockRule{id: "dynamic"})

	if count := eng.RuleCount(); count != 1 {
		t.Errorf("Expected 1 rule after add, got %d", count)
	}
}

func TestEngine_MatchingRuleFiresAlert(t *testing.T) {
	ip := "1.2.3.4"
	alertToFire := &rules.Alert{
		RuleID:   "test-rule",
		Severity: rules.SeverityHigh,
		FiredAt:  time.Now(),
		SourceIP: &ip,
		EventIDs: []string{"evt-1"},
	}
	rule := &mockRule{
		id: "test-rule",
		matchFn: func(_ *rules.Event) *rules.Alert { return alertToFire },
	}

	eng, alertCh := newTestEngine([]rules.Rule{rule})
	event := &rules.Event{EventID: "evt-1", Message: "test"}
	eng.Process(event)

	select {
	case alert := <-alertCh:
		if alert.RuleID != "test-rule" {
			t.Errorf("Expected rule_id=test-rule, got %s", alert.RuleID)
		}
		if alert.Severity != rules.SeverityHigh {
			t.Errorf("Expected High severity, got %s", alert.Severity)
		}
	case <-time.After(100 * time.Millisecond):
		t.Fatal("Expected alert on channel, got timeout")
	}
}

func TestEngine_NonMatchingRuleNoAlert(t *testing.T) {
	rule := &mockRule{
		id:      "no-match",
		matchFn: func(_ *rules.Event) *rules.Alert { return nil },
	}
	eng, alertCh := newTestEngine([]rules.Rule{rule})
	eng.Process(&rules.Event{Message: "test"})

	select {
	case alert := <-alertCh:
		t.Errorf("Expected no alert, got %+v", alert)
	case <-time.After(50 * time.Millisecond):
		// correct — no alert
	}
}

func TestEngine_MultipleRulesAllFire(t *testing.T) {
	ip := "1.2.3.4"
	makeAlert := func(id string) *rules.Alert {
		return &rules.Alert{RuleID: id, Severity: rules.SeverityLow, FiredAt: time.Now(), EventIDs: []string{"e1"}, SourceIP: &ip}
	}
	rules1 := []rules.Rule{
		&mockRule{"r1", func(e *rules.Event) *rules.Alert { return makeAlert("r1") }},
		&mockRule{"r2", func(e *rules.Event) *rules.Alert { return makeAlert("r2") }},
		&mockRule{"r3", func(e *rules.Event) *rules.Alert { return makeAlert("r3") }},
	}

	eng, alertCh := newTestEngine(rules1)
	eng.Process(&rules.Event{Message: "test"})

	count := 0
	timeout := time.After(100 * time.Millisecond)
loop:
	for {
		select {
		case <-alertCh:
			count++
		case <-timeout:
			break loop
		}
	}
	if count != 3 {
		t.Errorf("Expected 3 alerts, got %d", count)
	}
}

func TestEngine_ProcessRaw_ValidJSON(t *testing.T) {
	ip := "9.9.9.9"
	fired := false
	rule := &mockRule{
		id: "raw-test",
		matchFn: func(e *rules.Event) *rules.Alert {
			if e.EventID == "test-raw-id" {
				fired = true
				return &rules.Alert{
					RuleID: "raw-test", Severity: rules.SeverityLow,
					FiredAt: time.Now(), EventIDs: []string{e.EventID}, SourceIP: &ip,
				}
			}
			return nil
		},
	}
	eng, alertCh := newTestEngine([]rules.Rule{rule})

	event := rules.Event{
		EventID:   "test-raw-id",
		Message:   "raw event",
		Timestamp: time.Now(),
	}
	payload, _ := json.Marshal(event)
	eng.ProcessRaw(payload)

	select {
	case <-alertCh:
		if !fired {
			t.Error("Rule was not invoked")
		}
	case <-time.After(100 * time.Millisecond):
		t.Fatal("Expected alert from raw event processing")
	}
}

func TestEngine_ProcessRaw_InvalidJSON(t *testing.T) {
	rule := &mockRule{id: "noop"}
	eng, alertCh := newTestEngine([]rules.Rule{rule})

	eng.ProcessRaw([]byte("{invalid json"))

	select {
	case alert := <-alertCh:
		t.Errorf("Expected no alert for invalid JSON, got %+v", alert)
	case <-time.After(50 * time.Millisecond):
		// correct — no alert, just error logged
	}
}

func TestEngine_AlertChannelFullDropsAlert(t *testing.T) {
	ip := "1.1.1.1"
	// Канал с нулевым буфером — всегда переполнен
	alertCh := make(chan *rules.Alert, 0)
	logger := zap.NewNop()

	rule := &mockRule{
		id: "overflow",
		matchFn: func(_ *rules.Event) *rules.Alert {
			return &rules.Alert{RuleID: "overflow", Severity: rules.SeverityLow, FiredAt: time.Now(), EventIDs: []string{"e1"}, SourceIP: &ip}
		},
	}
	eng := New([]rules.Rule{rule}, nil, nil, alertCh, logger)

	// Не должен паниковать или блокироваться
	eng.Process(&rules.Event{Message: "overflow test"})
}
