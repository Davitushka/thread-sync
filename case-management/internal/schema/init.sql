-- SOC case management — PostgreSQL (OLTP)

CREATE TABLE IF NOT EXISTS cases (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_number BIGINT GENERATED ALWAYS AS IDENTITY,
    title TEXT NOT NULL,
    description TEXT NOT NULL DEFAULT '',
    severity TEXT NOT NULL CHECK (severity IN ('low', 'medium', 'high', 'critical')),
    status TEXT NOT NULL CHECK (status IN ('new', 'triaged', 'investigating', 'contained', 'resolved', 'closed')),
    priority SMALLINT NOT NULL DEFAULT 2 CHECK (priority BETWEEN 1 AND 4),
    assignee TEXT,
    tags TEXT[] NOT NULL DEFAULT '{}',
    resolution TEXT CHECK (
        resolution IS NULL
        OR resolution IN ('true_positive', 'false_positive', 'benign', 'informational', 'other')
    ),
    resolution_notes TEXT,
    source TEXT NOT NULL DEFAULT 'manual' CHECK (source IN ('manual', 'alertmanager', 'api')),
    created_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    updated_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    closed_at TIMESTAMPTZ
);

CREATE INDEX IF NOT EXISTS idx_cases_status ON cases (status);
CREATE INDEX IF NOT EXISTS idx_cases_severity ON cases (severity);
CREATE INDEX IF NOT EXISTS idx_cases_assignee ON cases (assignee);
CREATE INDEX IF NOT EXISTS idx_cases_created ON cases (created_at DESC);

CREATE TABLE IF NOT EXISTS case_timeline (
    id UUID PRIMARY KEY DEFAULT gen_random_uuid(),
    case_id UUID NOT NULL REFERENCES cases (id) ON DELETE CASCADE,
    actor TEXT NOT NULL DEFAULT 'system',
    entry_type TEXT NOT NULL CHECK (
        entry_type IN ('comment', 'system', 'status', 'assignment', 'alert', 'event', 'field_change')
    ),
    body TEXT,
    metadata JSONB NOT NULL DEFAULT '{}',
    created_at TIMESTAMPTZ NOT NULL DEFAULT now()
);

CREATE INDEX IF NOT EXISTS idx_timeline_case ON case_timeline (case_id, created_at DESC);

CREATE TABLE IF NOT EXISTS case_linked_alerts (
    case_id UUID NOT NULL REFERENCES cases (id) ON DELETE CASCADE,
    fingerprint TEXT NOT NULL,
    rule_id TEXT,
    rule_title TEXT,
    severity TEXT,
    description TEXT,
    first_seen_at TIMESTAMPTZ NOT NULL,
    last_seen_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (case_id, fingerprint)
);

CREATE INDEX IF NOT EXISTS idx_linked_alerts_fp ON case_linked_alerts (fingerprint);

CREATE TABLE IF NOT EXISTS case_linked_events (
    case_id UUID NOT NULL REFERENCES cases (id) ON DELETE CASCADE,
    event_id UUID NOT NULL,
    note TEXT,
    linked_at TIMESTAMPTZ NOT NULL DEFAULT now(),
    PRIMARY KEY (case_id, event_id)
);
