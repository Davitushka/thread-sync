package rules

import (
	"fmt"
	"regexp"
	"strings"
	"time"
)

// SQLInjectionRule детектирует SQLi / NoSQL injection попытки в логах.
// Sigma: sigma-rules/sql_injection.yaml
// MITRE: T1190 (Exploit Public-Facing Application), T1059.007 (JS injection)
//
// Сигнатурный подход: regexp по message + url_path.
// Намеренно ложноположительный — настраивается через FalsePositivePatterns.
type SQLInjectionRule struct {
	sqlPatterns      []*regexp.Regexp
	nosqlPatterns    []*regexp.Regexp
	falsePositives   []*regexp.Regexp
}

// SQL injection паттерны — базовые сигнатуры.
var sqlSignatures = []string{
	// Классические SQL
	`(?i)(union\s+select|union\s+all\s+select)`,
	`(?i)('|")\s*(or|and)\s*('|")?\s*\d+\s*=\s*\d+`,
	`(?i);\s*(drop|alter|truncate|create)\s+(table|database)`,
	`(?i)exec(\s|\+)+(x?p_|sp_)\w+`,
	`(?i)information_schema\.(tables|columns)`,
	`(?i)(sleep|benchmark|waitfor\s+delay)\s*\(`,
	`(?i)0x[0-9a-fA-F]{4,}`,              // hex encoding
	`(?i)\bconvert\s*\(\s*int\s*,`,        // MSSQL error-based
	`(?i)char\s*\(\s*\d+\s*\)`,           // char() encoding
}

// NoSQL injection паттерны (MongoDB, Redis).
var nosqlSignatures = []string{
	`(?i)\$where\s*:`,                       // MongoDB $where
	`(?i)\$gt\s*:\s*(0|null|"")`,            // comparison ops
	`(?i)\$regex\s*:`,
	`(?i)\}\s*,\s*\{.*\$`,                   // operator injection
	`(?i)\/\*.*\*\/`,                        // comment injection
}

// Паттерны для фильтрации false positives.
var fpPatterns = []string{
	`(?i)health.check`,
	`(?i)swagger`,
	`(?i)actuator`,
}

func NewSQLInjectionRule() *SQLInjectionRule {
	r := &SQLInjectionRule{}
	for _, sig := range sqlSignatures {
		r.sqlPatterns = append(r.sqlPatterns, regexp.MustCompile(sig))
	}
	for _, sig := range nosqlSignatures {
		r.nosqlPatterns = append(r.nosqlPatterns, regexp.MustCompile(sig))
	}
	for _, fp := range fpPatterns {
		r.falsePositives = append(r.falsePositives, regexp.MustCompile(fp))
	}
	return r
}

func (r *SQLInjectionRule) ID() string {
	return "sql_injection_attempt"
}

func (r *SQLInjectionRule) Title() string {
	return "SQL/NoSQL Injection Attempt Detected in Application Logs"
}

// Match — stateless проверка сигнатур в message и url_path.
func (r *SQLInjectionRule) Match(event *Event) *Alert {
	// Проверяем только application и database события
	if event.SourceType != "dotnet" && event.SourceType != "postgresql" {
		return nil
	}

	target := event.Message
	if event.URLPath != nil {
		target += " " + *event.URLPath
	}

	// Исключаем false positives
	for _, fp := range r.falsePositives {
		if fp.MatchString(target) {
			return nil
		}
	}

	// Ищем SQL паттерны
	var matched []string
	for _, pat := range r.sqlPatterns {
		if pat.MatchString(target) {
			matched = append(matched, pat.String())
		}
	}
	// Ищем NoSQL паттерны
	for _, pat := range r.nosqlPatterns {
		if pat.MatchString(target) {
			matched = append(matched, "nosql:"+pat.String())
		}
	}

	if len(matched) == 0 {
		return nil
	}

	// Определяем severity: если ошибка БД — выше вероятность реального injection
	severity := SeverityHigh
	if event.SourceType == "postgresql" || (event.StatusCode != nil && *event.StatusCode == 500) {
		severity = SeverityCritical
	}

	var matchedShort []string
	for _, m := range matched {
		matchedShort = append(matchedShort, truncate(m, 40))
	}

	return &Alert{
		RuleID:    r.ID(),
		RuleTitle: r.Title(),
		Severity:  severity,
		Description: fmt.Sprintf(
			"SQL/NoSQL injection attempt detected: %d pattern(s) matched in %s event",
			len(matched), event.SourceType,
		),
		SourceIP:  event.SourceIP,
		UserID:    event.UserID,
		EventIDs:  []string{event.EventID},
		MitreTags: []string{"T1190", "T1059.007"},
		FiredAt:   time.Now().UTC(),
		Context: map[string]any{
			"matched_patterns": matchedShort,
			"source_type":      event.SourceType,
			"url_path":         StrVal(event.URLPath),
			"status_code":      IntVal(event.StatusCode),
		},
	}
}

func truncate(s string, n int) string {
	if len(s) <= n {
		return s
	}
	return s[:n] + "..."
}

// trimParens убирает скобки из SQL функций для читаемости в логах.
func trimParens(s string) string {
	return strings.TrimRight(s, "(")
}

var _ = trimParens // используется в будущем для pretty-print сигнатур
