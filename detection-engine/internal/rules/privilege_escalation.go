package rules

import (
	"fmt"
	"strings"
	"time"
)

// PrivilegeEscalationRule детектирует privilege escalation и доступ к admin-эндпоинтам.
// Sigma: sigma-rules/privilege_escalation.yaml
// MITRE: T1068 (Exploitation for Privilege Escalation), T1078.003 (Local Accounts)
//
// Комбинирует stateless (URL path) и stateful (количество попыток) логику.
type PrivilegeEscalationRule struct {
	// adminPaths — паттерны путей, требующих прав администратора
	adminPaths []string
	// Threshold для stateful: количество попыток доступа к admin за window
	Threshold int
}

func NewPrivilegeEscalationRule() *PrivilegeEscalationRule {
	return &PrivilegeEscalationRule{
		adminPaths: []string{
			"/api/admin",
			"/api/internal",
			"/api/management",
			"/admin",
			"/manage",
			"/actuator",
			"/api/users/roles",
			"/api/permissions",
			"/api/audit",
		},
		Threshold: 3,
	}
}

func (r *PrivilegeEscalationRule) ID() string {
	return "privilege_escalation_attempt"
}

func (r *PrivilegeEscalationRule) Title() string {
	return "Privilege Escalation or Unauthorized Admin Access Attempt"
}

// isAdminPath проверяет, является ли path административным.
func (r *PrivilegeEscalationRule) isAdminPath(path string) bool {
	lower := strings.ToLower(path)
	for _, p := range r.adminPaths {
		if strings.HasPrefix(lower, strings.ToLower(p)) {
			return true
		}
	}
	return false
}

// Match — stateless проверка: 403 на admin path без роли admin.
func (r *PrivilegeEscalationRule) Match(event *Event) *Alert {
	if event.URLPath == nil || event.StatusCode == nil {
		return nil
	}

	// Только HTTP события
	if event.EventType != "application" && event.EventType != "auth" {
		return nil
	}

	if !r.isAdminPath(*event.URLPath) {
		return nil
	}

	code := *event.StatusCode

	// Case 1: 403 Forbidden — пользователь достучался, но прав нет
	if code == 403 {
		return r.buildAlert(event, "unauthorized_access",
			fmt.Sprintf("Access denied (403) to admin endpoint: %s", *event.URLPath),
			SeverityHigh)
	}

	// Case 2: 200/201 на admin endpoint для пользователя без роли admin
	// (ролевая информация передаётся через metadata)
	if code >= 200 && code < 300 {
		role, _ := event.Metadata["UserRole"].(string)
		if role != "" && role != "admin" && role != "superadmin" {
			return r.buildAlert(event, "role_bypass",
				fmt.Sprintf("Non-admin user (role=%s) accessed admin endpoint: %s", role, *event.URLPath),
				SeverityCritical)
		}
	}

	// Case 3: Попытка изменить роль (PATCH/PUT на /roles endpoint)
	if (code >= 200 && code < 300) || code == 400 {
		method := StrVal(event.HTTPMethod)
		path := *event.URLPath
		if (method == "PUT" || method == "PATCH" || method == "POST") &&
			(strings.Contains(path, "/roles") || strings.Contains(path, "/permissions")) {
			return r.buildAlert(event, "role_modification",
				fmt.Sprintf("Role/permission modification attempt: %s %s (status=%d)", method, path, code),
				SeverityCritical)
		}
	}

	return nil
}

// Evaluate — stateful: счётчик попыток доступа к admin endpoints.
func (r *PrivilegeEscalationRule) Evaluate(event *Event, state StateStore) *Alert {
	if event.SourceIP == nil || event.URLPath == nil {
		return nil
	}
	if !r.isAdminPath(*event.URLPath) {
		return nil
	}
	if event.StatusCode == nil || *event.StatusCode != 403 {
		return nil
	}

	key := fmt.Sprintf("priv:%s", *event.SourceIP)
	count, err := state.Increment(key, 5*time.Minute)
	if err != nil {
		return nil
	}

	if count != int64(r.Threshold) {
		return nil
	}

	ip := *event.SourceIP
	return &Alert{
		RuleID:    r.ID(),
		RuleTitle: r.Title(),
		Severity:  SeverityCritical,
		Description: fmt.Sprintf(
			"Repeated privilege escalation attempts: %d forbidden requests to admin endpoints from %s",
			r.Threshold, ip,
		),
		SourceIP:  &ip,
		UserID:    event.UserID,
		EventIDs:  []string{event.EventID},
		MitreTags: []string{"T1068", "T1078.003", "T1548"},
		FiredAt:   time.Now().UTC(),
		Context: map[string]any{
			"attempt_count": count,
			"url_path":      *event.URLPath,
			"window":        "5m",
		},
	}
}

func (r *PrivilegeEscalationRule) buildAlert(
	event *Event,
	subtype string,
	description string,
	severity AlertSeverity,
) *Alert {
	return &Alert{
		RuleID:      r.ID(),
		RuleTitle:   r.Title(),
		Severity:    severity,
		Description: description,
		SourceIP:    event.SourceIP,
		UserID:      event.UserID,
		EventIDs:    []string{event.EventID},
		MitreTags:   []string{"T1068", "T1078.003", "T1548"},
		FiredAt:     time.Now().UTC(),
		Context: map[string]any{
			"subtype":    subtype,
			"url_path":   StrVal(event.URLPath),
			"user_role":  event.Metadata["UserRole"],
			"http_method": StrVal(event.HTTPMethod),
		},
	}
}
