package server

import (
	"context"
	"encoding/json"
	"net/http"
	"strings"
	"time"

	"github.com/siem-lite/case-management/internal/models"
	"github.com/siem-lite/case-management/internal/store"
)

// AlertmanagerWebhook matches Prometheus Alertmanager webhook payload.
type AlertmanagerWebhook struct {
	Version  string              `json:"version"`
	GroupKey string              `json:"groupKey"`
	Status   string              `json:"status"`
	Alerts   []AlertmanagerAlert `json:"alerts"`
}

type AlertmanagerAlert struct {
	Status       string            `json:"status"`
	Labels       map[string]string `json:"labels"`
	Annotations  map[string]string `json:"annotations"`
	StartsAt     string            `json:"startsAt"`
	EndsAt       string            `json:"endsAt"`
	GeneratorURL string            `json:"generatorURL"`
	Fingerprint  string            `json:"fingerprint"`
}

func severityRank(s string) int {
	switch strings.ToLower(s) {
	case "critical":
		return 4
	case "high", "warning":
		return 3
	case "medium":
		return 2
	case "low":
		return 1
	default:
		return 2
	}
}

func mapAlertSeverity(label string) string {
	switch strings.ToLower(label) {
	case "critical":
		return "critical"
	case "high", "warning":
		return "high"
	case "medium":
		return "medium"
	default:
		return "low"
	}
}

func (s *Server) handleAlertmanager(w http.ResponseWriter, r *http.Request) {
	var payload AlertmanagerWebhook
	if err := json.NewDecoder(r.Body).Decode(&payload); err != nil {
		http.Error(w, `{"error":"invalid json"}`, http.StatusBadRequest)
		return
	}
	minRank := severityRank(s.autoMinSeverity)
	var firingNew, firingLinked, resolvedNotes, skipped int

	for _, a := range payload.Alerts {
		fp := strings.TrimSpace(a.Fingerprint)
		if fp == "" {
			fp = fallbackFingerprint(a)
		}
		if fp == "" {
			skipped++
			continue
		}
		sev := mapAlertSeverity(a.Labels["severity"])
		if severityRank(sev) < minRank {
			skipped++
			continue
		}
		switch a.Status {
		case "firing":
			if !s.autoFromAlerts {
				skipped++
				continue
			}
			isNew, err := s.ingestFiring(r.Context(), a, fp, sev, payload.GroupKey)
			if err != nil {
				s.log.Error("alertmanager firing", "err", err, "fingerprint", fp)
				http.Error(w, `{"error":"ingest failed"}`, http.StatusInternalServerError)
				return
			}
			if isNew {
				firingNew++
			} else {
				firingLinked++
			}
		case "resolved":
			err := s.ingestResolved(r.Context(), fp, a)
			if err == store.ErrNotFound {
				skipped++
			} else if err != nil {
				s.log.Error("alertmanager resolved", "err", err, "fingerprint", fp)
			} else {
				resolvedNotes++
			}
		}
	}

	w.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(w).Encode(map[string]int{
		"firing_new_cases":       firingNew,
		"firing_linked_existing": firingLinked,
		"resolved_timeline":      resolvedNotes,
		"skipped":                skipped,
	})
}

func fallbackFingerprint(a AlertmanagerAlert) string {
	parts := []string{
		a.Labels["alertname"],
		a.Labels["rule_id"],
		a.Labels["source_ip"],
		a.Labels["instance"],
	}
	return strings.Join(parts, "|")
}

// ingestFiring returns true if a new case was created.
func (s *Server) ingestFiring(ctx context.Context, a AlertmanagerAlert, fp, sev, groupKey string) (bool, error) {
	seenAt := time.Now().UTC()
	caseID, err := s.db.FindActiveCaseByFingerprint(ctx, fp)
	if err == nil {
		desc := alertDescription(a)
		ruleID := a.Labels["rule_id"]
		if ruleID == "" {
			ruleID = a.Labels["alertname"]
		}
		title := a.Labels["alertname"]
		rid := ruleID
		tit := title
		sevPtr := sev
		var d *string
		if desc != "" {
			d = &desc
		}
		if err := s.db.UpsertLinkedAlert(ctx, caseID, fp, &rid, &tit, &sevPtr, d, seenAt); err != nil {
			return false, err
		}
		_, _ = s.db.AddTimeline(ctx, caseID, s.defaultActor, "alert", ptr("Related alert fired again"), map[string]any{
			"fingerprint": fp,
			"rule_id":     ruleID,
			"severity":    sev,
		})
		return false, nil
	}
	if err != store.ErrNotFound {
		return false, err
	}

	title := firstNonEmpty(
		a.Annotations["summary"],
		a.Labels["alertname"],
		"Security alert",
	)
	desc := alertDescription(a)
	ruleID := a.Labels["rule_id"]
	if ruleID == "" {
		ruleID = a.Labels["alertname"]
	}
	req := models.CreateCaseRequest{
		Title:       title,
		Description: desc,
		Severity:    sev,
		Status:      "new",
		Priority:    priorityFromSeverity(sev),
		Source:      "alertmanager",
		Tags:        []string{"auto", "alertmanager"},
	}
	c, err := s.db.CreateCase(ctx, req)
	if err != nil {
		return false, err
	}
	meta := map[string]any{"fingerprint": fp}
	if groupKey != "" {
		meta["group_key"] = groupKey
	}
	_, _ = s.db.AddTimeline(ctx, c.ID, s.defaultActor, "system", ptr("Case opened from Alertmanager"), meta)
	rid := ruleID
	tit := a.Labels["alertname"]
	sevPtr := sev
	var d *string
	if desc != "" {
		d = &desc
	}
	if err := s.db.UpsertLinkedAlert(ctx, c.ID, fp, &rid, &tit, &sevPtr, d, seenAt); err != nil {
		return false, err
	}
	return true, nil
}

func alertDescription(a AlertmanagerAlert) string {
	if d := strings.TrimSpace(a.Annotations["description"]); d != "" {
		return d
	}
	return strings.TrimSpace(a.Annotations["summary"])
}

func firstNonEmpty(vals ...string) string {
	for _, v := range vals {
		if strings.TrimSpace(v) != "" {
			return strings.TrimSpace(v)
		}
	}
	return ""
}

func priorityFromSeverity(sev string) int16 {
	switch sev {
	case "critical":
		return 1
	case "high":
		return 2
	case "medium":
		return 3
	default:
		return 4
	}
}

func (s *Server) ingestResolved(ctx context.Context, fp string, a AlertmanagerAlert) error {
	caseID, err := s.db.FindLatestCaseByFingerprint(ctx, fp)
	if err != nil {
		return err
	}
	rule := a.Labels["alertname"]
	_, err = s.db.AddTimeline(ctx, caseID, s.defaultActor, "system", ptr("Alert resolved in Alertmanager"), map[string]any{
		"fingerprint": fp,
		"rule":        rule,
		"ends_at":     a.EndsAt,
	})
	return err
}
