-- Зрелость процесса SOC + контекст для расследования

ALTER TABLE cases
    ADD COLUMN IF NOT EXISTS acknowledged_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS due_at TIMESTAMPTZ,
    ADD COLUMN IF NOT EXISTS runbook_url TEXT;

ALTER TABLE case_linked_alerts
    ADD COLUMN IF NOT EXISTS context JSONB NOT NULL DEFAULT '{}';

ALTER TABLE case_timeline DROP CONSTRAINT IF EXISTS case_timeline_entry_type_check;
ALTER TABLE case_timeline ADD CONSTRAINT case_timeline_entry_type_check CHECK (
    entry_type IN (
        'comment',
        'system',
        'status',
        'assignment',
        'alert',
        'event',
        'field_change',
        'ack',
        'runbook',
        'data_note'
    )
);
