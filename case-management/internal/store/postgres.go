package store

import (
	"context"
	"encoding/json"
	"errors"
	"fmt"
	"strings"
	"time"

	"github.com/google/uuid"
	"github.com/jackc/pgx/v5"
	"github.com/jackc/pgx/v5/pgxpool"
	"github.com/siem-lite/case-management/internal/models"
)

var ErrNotFound = errors.New("not found")

type Postgres struct {
	pool *pgxpool.Pool
}

func NewPostgres(ctx context.Context, databaseURL string) (*Postgres, error) {
	cfg, err := pgxpool.ParseConfig(databaseURL)
	if err != nil {
		return nil, err
	}
	cfg.MaxConns = 16
	cfg.MinConns = 1
	pool, err := pgxpool.NewWithConfig(ctx, cfg)
	if err != nil {
		return nil, err
	}
	if err := pool.Ping(ctx); err != nil {
		pool.Close()
		return nil, err
	}
	return &Postgres{pool: pool}, nil
}

func (p *Postgres) Close() { p.pool.Close() }

func (p *Postgres) Migrate(ctx context.Context, sql string) error {
	_, err := p.pool.Exec(ctx, sql)
	return err
}

func displayKey(n int64) string {
	return fmt.Sprintf("INC-%d", n)
}

type ListFilter struct {
	Status   string
	Severity string
	Assignee string
	Query    string
	Limit    int
	Offset   int
}

func (p *Postgres) ListCases(ctx context.Context, f ListFilter) ([]models.Case, int64, error) {
	if f.Limit <= 0 || f.Limit > 500 {
		f.Limit = 50
	}
	if f.Offset < 0 {
		f.Offset = 0
	}
	var conds []string
	var args []any
	i := 1
	if f.Status != "" {
		conds = append(conds, fmt.Sprintf("status = $%d", i))
		args = append(args, f.Status)
		i++
	}
	if f.Severity != "" {
		conds = append(conds, fmt.Sprintf("severity = $%d", i))
		args = append(args, f.Severity)
		i++
	}
	if f.Assignee != "" {
		conds = append(conds, fmt.Sprintf("assignee = $%d", i))
		args = append(args, f.Assignee)
		i++
	}
	if f.Query != "" {
		conds = append(conds, fmt.Sprintf("(title ILIKE $%d OR description ILIKE $%d)", i, i))
		args = append(args, "%"+f.Query+"%")
		i++
	}
	where := ""
	if len(conds) > 0 {
		where = "WHERE " + strings.Join(conds, " AND ")
	}
	countSQL := "SELECT count(*) FROM cases " + where
	var total int64
	if err := p.pool.QueryRow(ctx, countSQL, args...).Scan(&total); err != nil {
		return nil, 0, err
	}
	args = append(args, f.Limit, f.Offset)
	listSQL := fmt.Sprintf(`
		SELECT id, case_number, title, description, severity, status, priority, assignee, tags,
		       resolution, resolution_notes, source, created_at, updated_at, closed_at
		FROM cases %s
		ORDER BY created_at DESC
		LIMIT $%d OFFSET $%d`, where, i, i+1)
	rows, err := p.pool.Query(ctx, listSQL, args...)
	if err != nil {
		return nil, 0, err
	}
	defer rows.Close()
	var out []models.Case
	for rows.Next() {
		var c models.Case
		var assignee, resolution, resNotes *string
		if err := rows.Scan(
			&c.ID, &c.CaseNumber, &c.Title, &c.Description, &c.Severity, &c.Status, &c.Priority,
			&assignee, &c.Tags, &resolution, &resNotes, &c.Source,
			&c.CreatedAt, &c.UpdatedAt, &c.ClosedAt,
		); err != nil {
			return nil, 0, err
		}
		c.DisplayKey = displayKey(c.CaseNumber)
		c.Assignee = assignee
		c.Resolution = resolution
		c.ResolutionNotes = resNotes
		out = append(out, c)
	}
	return out, total, rows.Err()
}

func (p *Postgres) GetCase(ctx context.Context, id uuid.UUID) (*models.Case, error) {
	const q = `
		SELECT id, case_number, title, description, severity, status, priority, assignee, tags,
		       resolution, resolution_notes, source, created_at, updated_at, closed_at
		FROM cases WHERE id = $1`
	var c models.Case
	var assignee, resolution, resNotes *string
	err := p.pool.QueryRow(ctx, q, id).Scan(
		&c.ID, &c.CaseNumber, &c.Title, &c.Description, &c.Severity, &c.Status, &c.Priority,
		&assignee, &c.Tags, &resolution, &resNotes, &c.Source,
		&c.CreatedAt, &c.UpdatedAt, &c.ClosedAt,
	)
	if errors.Is(err, pgx.ErrNoRows) {
		return nil, ErrNotFound
	}
	if err != nil {
		return nil, err
	}
	c.DisplayKey = displayKey(c.CaseNumber)
	c.Assignee = assignee
	c.Resolution = resolution
	c.ResolutionNotes = resNotes
	return &c, nil
}

func (p *Postgres) CreateCase(ctx context.Context, req models.CreateCaseRequest) (*models.Case, error) {
	if req.Source == "" {
		req.Source = "manual"
	}
	if req.Status == "" {
		req.Status = "new"
	}
	if req.Severity == "" {
		req.Severity = "medium"
	}
	if req.Priority == 0 {
		req.Priority = 2
	}
	const q = `
		INSERT INTO cases (title, description, severity, status, priority, assignee, tags, source)
		VALUES ($1,$2,$3,$4,$5,$6,$7,$8)
		RETURNING id, case_number, title, description, severity, status, priority, assignee, tags,
		          resolution, resolution_notes, source, created_at, updated_at, closed_at`
	var c models.Case
	var assignee, resolution, resNotes *string
	err := p.pool.QueryRow(ctx, q,
		req.Title, req.Description, req.Severity, req.Status, req.Priority, req.Assignee, req.Tags, req.Source,
	).Scan(
		&c.ID, &c.CaseNumber, &c.Title, &c.Description, &c.Severity, &c.Status, &c.Priority,
		&assignee, &c.Tags, &resolution, &resNotes, &c.Source,
		&c.CreatedAt, &c.UpdatedAt, &c.ClosedAt,
	)
	if err != nil {
		return nil, err
	}
	c.DisplayKey = displayKey(c.CaseNumber)
	c.Assignee = assignee
	c.Resolution = resolution
	c.ResolutionNotes = resNotes
	return &c, nil
}

func (p *Postgres) PatchCase(ctx context.Context, id uuid.UUID, req models.PatchCaseRequest) (*models.Case, error) {
	cur, err := p.GetCase(ctx, id)
	if err != nil {
		return nil, err
	}
	title := cur.Title
	desc := cur.Description
	sev := cur.Severity
	st := cur.Status
	pr := cur.Priority
	var assignee = cur.Assignee
	tags := cur.Tags
	var resolution = cur.Resolution
	var resNotes = cur.ResolutionNotes
	if req.Title != nil {
		title = *req.Title
	}
	if req.Description != nil {
		desc = *req.Description
	}
	if req.Severity != nil {
		sev = *req.Severity
	}
	if req.Status != nil {
		st = *req.Status
	}
	if req.Priority != nil {
		pr = *req.Priority
	}
	if req.Assignee != nil {
		if *req.Assignee == "" {
			assignee = nil
		} else {
			v := *req.Assignee
			assignee = &v
		}
	}
	if req.Tags != nil {
		tags = req.Tags
	}
	if req.Resolution != nil {
		if *req.Resolution == "" {
			resolution = nil
		} else {
			v := *req.Resolution
			resolution = &v
		}
	}
	if req.ResolutionNotes != nil {
		if *req.ResolutionNotes == "" {
			resNotes = nil
		} else {
			v := *req.ResolutionNotes
			resNotes = &v
		}
	}
	var newClosedAt *time.Time
	if st == "closed" || st == "resolved" {
		if cur.ClosedAt != nil {
			newClosedAt = cur.ClosedAt
		} else {
			t := time.Now().UTC()
			newClosedAt = &t
		}
	}
	const q = `
		UPDATE cases SET
			title = $2, description = $3, severity = $4, status = $5, priority = $6,
			assignee = $7, tags = $8, resolution = $9, resolution_notes = $10,
			closed_at = $11,
			updated_at = now()
		WHERE id = $1
		RETURNING id, case_number, title, description, severity, status, priority, assignee, tags,
		          resolution, resolution_notes, source, created_at, updated_at, closed_at`
	var c models.Case
	var a, res, rn *string
	err = p.pool.QueryRow(ctx, q, id, title, desc, sev, st, pr, assignee, tags, resolution, resNotes, newClosedAt).Scan(
		&c.ID, &c.CaseNumber, &c.Title, &c.Description, &c.Severity, &c.Status, &c.Priority,
		&a, &c.Tags, &res, &rn, &c.Source, &c.CreatedAt, &c.UpdatedAt, &c.ClosedAt,
	)
	if errors.Is(err, pgx.ErrNoRows) {
		return nil, ErrNotFound
	}
	if err != nil {
		return nil, err
	}
	c.DisplayKey = displayKey(c.CaseNumber)
	c.Assignee = a
	c.Resolution = res
	c.ResolutionNotes = rn
	return &c, nil
}

func (p *Postgres) AddTimeline(ctx context.Context, caseID uuid.UUID, actor, entryType string, body *string, meta map[string]any) (*models.TimelineEntry, error) {
	var metaJSON []byte
	var err error
	if meta == nil {
		metaJSON = []byte("{}")
	} else {
		metaJSON, err = json.Marshal(meta)
		if err != nil {
			return nil, err
		}
	}
	const q = `
		INSERT INTO case_timeline (case_id, actor, entry_type, body, metadata)
		VALUES ($1,$2,$3,$4,$5)
		RETURNING id, case_id, actor, entry_type, body, metadata, created_at`
	var e models.TimelineEntry
	var b *string
	err = p.pool.QueryRow(ctx, q, caseID, actor, entryType, body, metaJSON).Scan(
		&e.ID, &e.CaseID, &e.Actor, &e.EntryType, &b, &e.Metadata, &e.CreatedAt,
	)
	e.Body = b
	return &e, err
}

func (p *Postgres) ListTimeline(ctx context.Context, caseID uuid.UUID) ([]models.TimelineEntry, error) {
	const q = `
		SELECT id, case_id, actor, entry_type, body, metadata, created_at
		FROM case_timeline WHERE case_id = $1 ORDER BY created_at ASC`
	rows, err := p.pool.Query(ctx, q, caseID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []models.TimelineEntry
	for rows.Next() {
		var e models.TimelineEntry
		var b *string
		if err := rows.Scan(&e.ID, &e.CaseID, &e.Actor, &e.EntryType, &b, &e.Metadata, &e.CreatedAt); err != nil {
			return nil, err
		}
		e.Body = b
		out = append(out, e)
	}
	return out, rows.Err()
}

func (p *Postgres) LinkEvent(ctx context.Context, caseID uuid.UUID, eventID uuid.UUID, note *string) error {
	const q = `
		INSERT INTO case_linked_events (case_id, event_id, note) VALUES ($1,$2,$3)
		ON CONFLICT (case_id, event_id) DO UPDATE SET note = COALESCE(EXCLUDED.note, case_linked_events.note)`
	_, err := p.pool.Exec(ctx, q, caseID, eventID, note)
	return err
}

func (p *Postgres) ListLinkedEvents(ctx context.Context, caseID uuid.UUID) ([]models.LinkedEvent, error) {
	const q = `SELECT event_id, note, linked_at FROM case_linked_events WHERE case_id = $1 ORDER BY linked_at`
	rows, err := p.pool.Query(ctx, q, caseID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []models.LinkedEvent
	for rows.Next() {
		var e models.LinkedEvent
		var note *string
		if err := rows.Scan(&e.EventID, &note, &e.LinkedAt); err != nil {
			return nil, err
		}
		e.Note = note
		out = append(out, e)
	}
	return out, rows.Err()
}

func (p *Postgres) UpsertLinkedAlert(ctx context.Context, caseID uuid.UUID, fp string, ruleID, ruleTitle, severity, description *string, seenAt time.Time) error {
	const q = `
		INSERT INTO case_linked_alerts (case_id, fingerprint, rule_id, rule_title, severity, description, first_seen_at, last_seen_at)
		VALUES ($1,$2,$3,$4,$5,$6,$7,$7)
		ON CONFLICT (case_id, fingerprint) DO UPDATE SET
			last_seen_at = EXCLUDED.last_seen_at,
			rule_id = COALESCE(EXCLUDED.rule_id, case_linked_alerts.rule_id),
			rule_title = COALESCE(EXCLUDED.rule_title, case_linked_alerts.rule_title),
			severity = COALESCE(EXCLUDED.severity, case_linked_alerts.severity),
			description = COALESCE(NULLIF(EXCLUDED.description, ''), case_linked_alerts.description)`
	_, err := p.pool.Exec(ctx, q, caseID, fp, ruleID, ruleTitle, severity, description, seenAt)
	return err
}

func (p *Postgres) ListLinkedAlerts(ctx context.Context, caseID uuid.UUID) ([]models.LinkedAlert, error) {
	const q = `
		SELECT fingerprint, rule_id, rule_title, severity, description, first_seen_at, last_seen_at
		FROM case_linked_alerts WHERE case_id = $1 ORDER BY first_seen_at`
	rows, err := p.pool.Query(ctx, q, caseID)
	if err != nil {
		return nil, err
	}
	defer rows.Close()
	var out []models.LinkedAlert
	for rows.Next() {
		var a models.LinkedAlert
		if err := rows.Scan(&a.Fingerprint, &a.RuleID, &a.RuleTitle, &a.Severity, &a.Description, &a.FirstSeenAt, &a.LastSeenAt); err != nil {
			return nil, err
		}
		out = append(out, a)
	}
	return out, rows.Err()
}

func (p *Postgres) FindLatestCaseByFingerprint(ctx context.Context, fingerprint string) (uuid.UUID, error) {
	const q = `
		SELECT c.id FROM cases c
		INNER JOIN case_linked_alerts la ON la.case_id = c.id
		WHERE la.fingerprint = $1
		ORDER BY c.updated_at DESC
		LIMIT 1`
	var id uuid.UUID
	err := p.pool.QueryRow(ctx, q, fingerprint).Scan(&id)
	if errors.Is(err, pgx.ErrNoRows) {
		return uuid.Nil, ErrNotFound
	}
	return id, err
}

func (p *Postgres) FindActiveCaseByFingerprint(ctx context.Context, fingerprint string) (uuid.UUID, error) {
	const q = `
		SELECT c.id FROM cases c
		INNER JOIN case_linked_alerts la ON la.case_id = c.id
		WHERE la.fingerprint = $1
		  AND c.status IN ('new','triaged','investigating','contained')
		ORDER BY c.created_at DESC
		LIMIT 1`
	var id uuid.UUID
	err := p.pool.QueryRow(ctx, q, fingerprint).Scan(&id)
	if errors.Is(err, pgx.ErrNoRows) {
		return uuid.Nil, ErrNotFound
	}
	return id, err
}

func (p *Postgres) GetCaseDetail(ctx context.Context, id uuid.UUID) (*models.CaseDetail, error) {
	c, err := p.GetCase(ctx, id)
	if err != nil {
		return nil, err
	}
	tl, err := p.ListTimeline(ctx, id)
	if err != nil {
		return nil, err
	}
	al, err := p.ListLinkedAlerts(ctx, id)
	if err != nil {
		return nil, err
	}
	ev, err := p.ListLinkedEvents(ctx, id)
	if err != nil {
		return nil, err
	}
	return &models.CaseDetail{Case: *c, Timeline: tl, Alerts: al, Events: ev}, nil
}
