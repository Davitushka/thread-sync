package models

import (
	"encoding/json"
	"time"

	"github.com/google/uuid"
)

type Case struct {
	ID              uuid.UUID  `json:"id"`
	CaseNumber      int64      `json:"case_number"`
	DisplayKey      string     `json:"display_key"`
	Title           string     `json:"title"`
	Description     string     `json:"description"`
	Severity        string     `json:"severity"`
	Status          string     `json:"status"`
	Priority        int16      `json:"priority"`
	Assignee        *string    `json:"assignee,omitempty"`
	Tags            []string   `json:"tags"`
	Resolution      *string    `json:"resolution,omitempty"`
	ResolutionNotes *string    `json:"resolution_notes,omitempty"`
	Source          string     `json:"source"`
	CreatedAt       time.Time  `json:"created_at"`
	UpdatedAt       time.Time  `json:"updated_at"`
	ClosedAt        *time.Time `json:"closed_at,omitempty"`
}

type TimelineEntry struct {
	ID        uuid.UUID       `json:"id"`
	CaseID    uuid.UUID       `json:"case_id"`
	Actor     string          `json:"actor"`
	EntryType string          `json:"entry_type"`
	Body      *string         `json:"body,omitempty"`
	Metadata  json.RawMessage `json:"metadata"`
	CreatedAt time.Time       `json:"created_at"`
}

type LinkedAlert struct {
	Fingerprint string    `json:"fingerprint"`
	RuleID      *string   `json:"rule_id,omitempty"`
	RuleTitle   *string   `json:"rule_title,omitempty"`
	Severity    *string   `json:"severity,omitempty"`
	Description *string   `json:"description,omitempty"`
	FirstSeenAt time.Time `json:"first_seen_at"`
	LastSeenAt  time.Time `json:"last_seen_at"`
}

type LinkedEvent struct {
	EventID  uuid.UUID `json:"event_id"`
	Note     *string   `json:"note,omitempty"`
	LinkedAt time.Time `json:"linked_at"`
}

type CaseDetail struct {
	Case
	Timeline []TimelineEntry `json:"timeline"`
	Alerts   []LinkedAlert   `json:"linked_alerts"`
	Events   []LinkedEvent   `json:"linked_events"`
}

type CreateCaseRequest struct {
	Title       string   `json:"title"`
	Description string   `json:"description"`
	Severity    string   `json:"severity"`
	Status      string   `json:"status"`
	Priority    int16    `json:"priority"`
	Assignee    *string  `json:"assignee,omitempty"`
	Tags        []string `json:"tags"`
	Source      string   `json:"source"`
}

type PatchCaseRequest struct {
	Title           *string  `json:"title,omitempty"`
	Description     *string  `json:"description,omitempty"`
	Severity        *string  `json:"severity,omitempty"`
	Status          *string  `json:"status,omitempty"`
	Priority        *int16   `json:"priority,omitempty"`
	Assignee        *string  `json:"assignee,omitempty"`
	Tags            []string `json:"tags,omitempty"`
	Resolution      *string  `json:"resolution,omitempty"`
	ResolutionNotes *string  `json:"resolution_notes,omitempty"`
}

type TimelineCreateRequest struct {
	Body string `json:"body"`
}

type LinkEventRequest struct {
	EventID uuid.UUID `json:"event_id"`
	Note    *string   `json:"note,omitempty"`
}

type LinkAlertRequest struct {
	Fingerprint string  `json:"fingerprint"`
	RuleID      *string `json:"rule_id,omitempty"`
	RuleTitle   *string `json:"rule_title,omitempty"`
	Severity    *string `json:"severity,omitempty"`
	Description *string `json:"description,omitempty"`
}
