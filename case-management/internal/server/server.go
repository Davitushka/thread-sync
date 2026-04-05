package server

import (
	"encoding/json"
	"log/slog"
	"net/http"
	"os"
	"strconv"
	"strings"
	"time"

	"github.com/go-chi/chi/v5"
	"github.com/go-chi/chi/v5/middleware"
	"github.com/google/uuid"
	"github.com/siem-lite/case-management/internal/models"
	"github.com/siem-lite/case-management/internal/store"
)

type Server struct {
	log             *slog.Logger
	db              *store.Postgres
	autoFromAlerts  bool
	autoMinSeverity string // critical | high | medium | low
	defaultActor    string
	grafanaBaseURL  string
}

func New(db *store.Postgres) *Server {
	auto := true
	switch strings.ToLower(strings.TrimSpace(os.Getenv("CASEMGMT_AUTO_CASE_FROM_ALERTS"))) {
	case "false", "0", "no":
		auto = false
	}
	minSev := strings.ToLower(strings.TrimSpace(os.Getenv("CASEMGMT_AUTO_CASE_MIN_SEVERITY")))
	if minSev == "" {
		minSev = "medium"
	}
	actor := strings.TrimSpace(os.Getenv("CASEMGMT_DEFAULT_ACTOR"))
	if actor == "" {
		actor = "system"
	}
	grafana := strings.TrimSpace(os.Getenv("CASEMGMT_GRAFANA_EXTERNAL_URL"))
	if grafana == "" {
		grafana = "http://localhost:3000"
	}
	return &Server{
		log:             slog.Default(),
		db:              db,
		autoFromAlerts:  auto,
		autoMinSeverity: minSev,
		defaultActor:    actor,
		grafanaBaseURL:  strings.TrimRight(grafana, "/"),
	}
}

func (s *Server) Router() *chi.Mux {
	r := chi.NewRouter()
	r.Use(middleware.RequestID)
	r.Use(middleware.RealIP)
	r.Use(middleware.Recoverer)
	r.Use(middleware.Logger)

	r.Get("/health", s.handleHealth)

	r.Route("/api/v1", func(r chi.Router) {
		r.Get("/cases", s.handleListCases)
		r.Post("/cases", s.handleCreateCase)
		r.Get("/cases/{id}", s.handleGetCase)
		r.Patch("/cases/{id}", s.handlePatchCase)
		r.Post("/cases/{id}/timeline", s.handleAddTimeline)
		r.Post("/cases/{id}/events", s.handleLinkEvent)
		r.Post("/cases/{id}/alerts", s.handleLinkAlert)
	})

	r.Post("/webhooks/alertmanager", s.handleAlertmanager)

	return r
}

func (s *Server) handleHealth(w http.ResponseWriter, _ *http.Request) {
	w.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(w).Encode(map[string]string{"status": "ok"})
}

func actorFromRequest(r *http.Request, fallback string) string {
	if v := strings.TrimSpace(r.Header.Get("X-SOC-Actor")); v != "" {
		return v
	}
	return fallback
}

func (s *Server) handleListCases(w http.ResponseWriter, r *http.Request) {
	q := r.URL.Query()
	lim, _ := strconv.Atoi(q.Get("limit"))
	off, _ := strconv.Atoi(q.Get("offset"))
	cases, total, err := s.db.ListCases(r.Context(), store.ListFilter{
		Status:   q.Get("status"),
		Severity: q.Get("severity"),
		Assignee: q.Get("assignee"),
		Query:    q.Get("q"),
		Limit:    lim,
		Offset:   off,
	})
	if err != nil {
		s.log.Error("list cases", "err", err)
		http.Error(w, `{"error":"list failed"}`, http.StatusInternalServerError)
		return
	}
	w.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(w).Encode(map[string]any{"cases": cases, "total": total})
}

func (s *Server) handleCreateCase(w http.ResponseWriter, r *http.Request) {
	var req models.CreateCaseRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, `{"error":"invalid json"}`, http.StatusBadRequest)
		return
	}
	if strings.TrimSpace(req.Title) == "" {
		http.Error(w, `{"error":"title required"}`, http.StatusBadRequest)
		return
	}
	if req.Severity == "" {
		req.Severity = "medium"
	}
	if req.Status == "" {
		req.Status = "new"
	}
	if err := validateSeverity(req.Severity); err != nil {
		http.Error(w, `{"error":"invalid severity"}`, http.StatusBadRequest)
		return
	}
	if err := validateStatus(req.Status); err != nil {
		http.Error(w, `{"error":"invalid status"}`, http.StatusBadRequest)
		return
	}
	if req.Priority == 0 {
		req.Priority = 2
	}
	if req.Source == "" {
		req.Source = "api"
	}
	c, err := s.db.CreateCase(r.Context(), req)
	if err != nil {
		s.log.Error("create case", "err", err)
		http.Error(w, `{"error":"create failed"}`, http.StatusInternalServerError)
		return
	}
	actor := actorFromRequest(r, s.defaultActor)
	_, _ = s.db.AddTimeline(r.Context(), c.ID, actor, "system", ptr("Case created"), map[string]any{
		"source": req.Source,
	})
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(http.StatusCreated)
	_ = json.NewEncoder(w).Encode(c)
}

func (s *Server) handleGetCase(w http.ResponseWriter, r *http.Request) {
	id, err := uuid.Parse(chi.URLParam(r, "id"))
	if err != nil {
		http.Error(w, `{"error":"invalid id"}`, http.StatusBadRequest)
		return
	}
	detail, err := s.db.GetCaseDetail(r.Context(), id)
	if err == store.ErrNotFound {
		http.Error(w, `{"error":"not found"}`, http.StatusNotFound)
		return
	}
	if err != nil {
		s.log.Error("get case", "err", err)
		http.Error(w, `{"error":"get failed"}`, http.StatusInternalServerError)
		return
	}
	w.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(w).Encode(detail)
}

func (s *Server) handlePatchCase(w http.ResponseWriter, r *http.Request) {
	id, err := uuid.Parse(chi.URLParam(r, "id"))
	if err != nil {
		http.Error(w, `{"error":"invalid id"}`, http.StatusBadRequest)
		return
	}
	var req models.PatchCaseRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, `{"error":"invalid json"}`, http.StatusBadRequest)
		return
	}
	if req.Severity != nil {
		if err := validateSeverity(*req.Severity); err != nil {
			http.Error(w, `{"error":"invalid severity"}`, http.StatusBadRequest)
			return
		}
	}
	if req.Status != nil {
		if err := validateStatus(*req.Status); err != nil {
			http.Error(w, `{"error":"invalid status"}`, http.StatusBadRequest)
			return
		}
	}
	if req.Resolution != nil && *req.Resolution != "" {
		if err := validateResolution(*req.Resolution); err != nil {
			http.Error(w, `{"error":"invalid resolution"}`, http.StatusBadRequest)
			return
		}
	}
	cur, err := s.db.GetCase(r.Context(), id)
	if err == store.ErrNotFound {
		http.Error(w, `{"error":"not found"}`, http.StatusNotFound)
		return
	}
	if err != nil {
		http.Error(w, `{"error":"get failed"}`, http.StatusInternalServerError)
		return
	}
	updated, err := s.db.PatchCase(r.Context(), id, req)
	if err != nil {
		s.log.Error("patch case", "err", err)
		http.Error(w, `{"error":"update failed"}`, http.StatusInternalServerError)
		return
	}
	actor := actorFromRequest(r, s.defaultActor)
	if req.Status != nil && *req.Status != cur.Status {
		meta := map[string]any{"from": cur.Status, "to": updated.Status}
		_, _ = s.db.AddTimeline(r.Context(), id, actor, "status", ptr("Status changed"), meta)
	}
	if req.Assignee != nil {
		var old, new string
		if cur.Assignee != nil {
			old = *cur.Assignee
		}
		if updated.Assignee != nil {
			new = *updated.Assignee
		}
		if old != new {
			_, _ = s.db.AddTimeline(r.Context(), id, actor, "assignment", ptr("Assignee updated"), map[string]any{
				"from": old, "to": new,
			})
		}
	}
	w.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(w).Encode(updated)
}

func (s *Server) handleAddTimeline(w http.ResponseWriter, r *http.Request) {
	id, err := uuid.Parse(chi.URLParam(r, "id"))
	if err != nil {
		http.Error(w, `{"error":"invalid id"}`, http.StatusBadRequest)
		return
	}
	var req models.TimelineCreateRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, `{"error":"invalid json"}`, http.StatusBadRequest)
		return
	}
	if strings.TrimSpace(req.Body) == "" {
		http.Error(w, `{"error":"body required"}`, http.StatusBadRequest)
		return
	}
	if _, err := s.db.GetCase(r.Context(), id); err == store.ErrNotFound {
		http.Error(w, `{"error":"not found"}`, http.StatusNotFound)
		return
	} else if err != nil {
		http.Error(w, `{"error":"get failed"}`, http.StatusInternalServerError)
		return
	}
	actor := actorFromRequest(r, s.defaultActor)
	e, err := s.db.AddTimeline(r.Context(), id, actor, "comment", &req.Body, nil)
	if err != nil {
		s.log.Error("timeline", "err", err)
		http.Error(w, `{"error":"insert failed"}`, http.StatusInternalServerError)
		return
	}
	w.Header().Set("Content-Type", "application/json")
	w.WriteHeader(http.StatusCreated)
	_ = json.NewEncoder(w).Encode(e)
}

func (s *Server) handleLinkEvent(w http.ResponseWriter, r *http.Request) {
	id, err := uuid.Parse(chi.URLParam(r, "id"))
	if err != nil {
		http.Error(w, `{"error":"invalid id"}`, http.StatusBadRequest)
		return
	}
	var req models.LinkEventRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, `{"error":"invalid json"}`, http.StatusBadRequest)
		return
	}
	if req.EventID == uuid.Nil {
		http.Error(w, `{"error":"event_id required"}`, http.StatusBadRequest)
		return
	}
	if _, err := s.db.GetCase(r.Context(), id); err == store.ErrNotFound {
		http.Error(w, `{"error":"not found"}`, http.StatusNotFound)
		return
	} else if err != nil {
		http.Error(w, `{"error":"get failed"}`, http.StatusInternalServerError)
		return
	}
	if err := s.db.LinkEvent(r.Context(), id, req.EventID, req.Note); err != nil {
		s.log.Error("link event", "err", err)
		http.Error(w, `{"error":"link failed"}`, http.StatusInternalServerError)
		return
	}
	actor := actorFromRequest(r, s.defaultActor)
	_, _ = s.db.AddTimeline(r.Context(), id, actor, "event", ptr("Event linked to case"), map[string]any{
		"event_id":    req.EventID.String(),
		"explore_url": s.grafanaBaseURL + "/explore",
	})
	w.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(w).Encode(map[string]string{"status": "linked"})
}

func (s *Server) handleLinkAlert(w http.ResponseWriter, r *http.Request) {
	id, err := uuid.Parse(chi.URLParam(r, "id"))
	if err != nil {
		http.Error(w, `{"error":"invalid id"}`, http.StatusBadRequest)
		return
	}
	var req models.LinkAlertRequest
	if err := json.NewDecoder(r.Body).Decode(&req); err != nil {
		http.Error(w, `{"error":"invalid json"}`, http.StatusBadRequest)
		return
	}
	fp := strings.TrimSpace(req.Fingerprint)
	if fp == "" {
		http.Error(w, `{"error":"fingerprint required"}`, http.StatusBadRequest)
		return
	}
	if _, err := s.db.GetCase(r.Context(), id); err == store.ErrNotFound {
		http.Error(w, `{"error":"not found"}`, http.StatusNotFound)
		return
	} else if err != nil {
		http.Error(w, `{"error":"get failed"}`, http.StatusInternalServerError)
		return
	}
	now := time.Now().UTC()
	if err := s.db.UpsertLinkedAlert(r.Context(), id, fp, req.RuleID, req.RuleTitle, req.Severity, req.Description, now); err != nil {
		s.log.Error("link alert", "err", err)
		http.Error(w, `{"error":"link failed"}`, http.StatusInternalServerError)
		return
	}
	actor := actorFromRequest(r, s.defaultActor)
	_, _ = s.db.AddTimeline(r.Context(), id, actor, "alert", ptr("Alert linked manually"), map[string]any{
		"fingerprint": fp,
	})
	w.Header().Set("Content-Type", "application/json")
	_ = json.NewEncoder(w).Encode(map[string]string{"status": "linked"})
}

func ptr(s string) *string { return &s }
