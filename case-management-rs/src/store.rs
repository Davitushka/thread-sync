use chrono::{DateTime, Duration, Utc};
use sqlx::postgres::PgPoolOptions;
use sqlx::{PgPool, Postgres, QueryBuilder, Row};
use uuid::Uuid;

use crate::models::*;

#[derive(Debug, thiserror::Error)]
pub enum StoreError {
    #[error("not found")]
    NotFound,
    #[error(transparent)]
    Sqlx(#[from] sqlx::Error),
}

pub struct ListFilter {
    pub status: String,
    pub severity: String,
    pub assignee: String,
    pub query: String,
    pub limit: i32,
    pub offset: i32,
}

#[derive(Clone)]
pub struct Store {
    pool: PgPool,
}

/// SLA: дедлайн расследования от момента создания кейса.
pub fn due_at_from_severity(sev: &str) -> Option<DateTime<Utc>> {
    let h = match sev.to_lowercase().as_str() {
        "critical" => 4i64,
        "high" => 8,
        "medium" => 24,
        _ => 72,
    };
    Some(Utc::now() + Duration::hours(h))
}

impl Store {
    pub async fn new(database_url: &str) -> Result<Self, sqlx::Error> {
        let pool = PgPoolOptions::new()
            .max_connections(16)
            .min_connections(1)
            .connect(database_url)
            .await?;
        sqlx::query("SELECT 1").execute(&pool).await?;
        Ok(Self { pool })
    }

    pub async fn migrate(&self, sql: &str) -> Result<(), sqlx::Error> {
        sqlx::raw_sql(sql).execute(&self.pool).await?;
        Ok(())
    }

    fn push_list_filters(qb: &mut QueryBuilder<'_, Postgres>, f: &ListFilter) {
        let mut has_where = false;
        if !f.status.is_empty() {
            qb.push(if has_where { " AND " } else { " WHERE " });
            has_where = true;
            qb.push("status = ").push_bind(f.status.clone());
        }
        if !f.severity.is_empty() {
            qb.push(if has_where { " AND " } else { " WHERE " });
            has_where = true;
            qb.push("severity = ").push_bind(f.severity.clone());
        }
        if !f.assignee.is_empty() {
            qb.push(if has_where { " AND " } else { " WHERE " });
            has_where = true;
            qb.push("assignee = ").push_bind(f.assignee.clone());
        }
        if !f.query.is_empty() {
            qb.push(if has_where { " AND " } else { " WHERE " });
            let pattern = format!("%{}%", f.query);
            qb.push("(title ILIKE ")
                .push_bind(pattern.clone())
                .push(" OR description ILIKE ")
                .push_bind(pattern)
                .push(")");
        }
    }

    pub async fn list_cases(&self, f: ListFilter) -> Result<(Vec<Case>, i64), StoreError> {
        let limit = if f.limit <= 0 || f.limit > 500 { 50i64 } else { f.limit as i64 };
        let offset = if f.offset < 0 { 0i64 } else { f.offset as i64 };

        let mut count_qb = QueryBuilder::<Postgres>::new("SELECT count(*) FROM cases");
        Self::push_list_filters(&mut count_qb, &f);
        let row = count_qb.build().fetch_one(&self.pool).await?;
        let total: i64 = row.get(0);

        let mut list_qb = QueryBuilder::<Postgres>::new(
            "SELECT id, case_number, title, description, severity, status, priority, \
             assignee, tags, resolution, resolution_notes, source, \
             created_at, updated_at, closed_at, acknowledged_at, due_at, runbook_url FROM cases",
        );
        Self::push_list_filters(&mut list_qb, &f);
        list_qb
            .push(" ORDER BY created_at DESC LIMIT ")
            .push_bind(limit)
            .push(" OFFSET ")
            .push_bind(offset);

        let mut cases: Vec<Case> = list_qb
            .build_query_as::<Case>()
            .fetch_all(&self.pool)
            .await?;
        for c in &mut cases {
            c.apply_display_key();
        }
        Ok((cases, total))
    }

    pub async fn get_case(&self, id: Uuid) -> Result<Case, StoreError> {
        let mut case = sqlx::query_as::<_, Case>(
            "SELECT id, case_number, title, description, severity, status, priority, \
             assignee, tags, resolution, resolution_notes, source, \
             created_at, updated_at, closed_at, acknowledged_at, due_at, runbook_url \
             FROM cases WHERE id = $1",
        )
        .bind(id)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)?;
        case.apply_display_key();
        Ok(case)
    }

    pub async fn create_case(&self, mut req: CreateCaseRequest) -> Result<Case, StoreError> {
        if req.source.is_empty() {
            req.source = "manual".into();
        }
        if req.status.is_empty() {
            req.status = "new".into();
        }
        if req.severity.is_empty() {
            req.severity = "medium".into();
        }
        if req.priority == 0 {
            req.priority = 2;
        }
        let due_at = due_at_from_severity(&req.severity);
        let runbook = req
            .runbook_url
            .as_ref()
            .map(|s| s.trim())
            .filter(|s| !s.is_empty())
            .map(|s| s.to_string());

        let mut case = sqlx::query_as::<_, Case>(
            "INSERT INTO cases (title, description, severity, status, priority, assignee, tags, source, due_at, runbook_url) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$8,$9,$10) \
             RETURNING id, case_number, title, description, severity, status, priority, \
                       assignee, tags, resolution, resolution_notes, source, \
                       created_at, updated_at, closed_at, acknowledged_at, due_at, runbook_url",
        )
        .bind(&req.title)
        .bind(&req.description)
        .bind(&req.severity)
        .bind(&req.status)
        .bind(req.priority)
        .bind(&req.assignee)
        .bind(&req.tags)
        .bind(&req.source)
        .bind(due_at)
        .bind(runbook)
        .fetch_one(&self.pool)
        .await?;
        case.apply_display_key();
        Ok(case)
    }

    pub async fn patch_case(&self, id: Uuid, req: PatchCaseRequest) -> Result<Case, StoreError> {
        let cur = self.get_case(id).await?;
        let prev_status = cur.status.clone();

        let title = req.title.unwrap_or(cur.title);
        let description = req.description.unwrap_or(cur.description);
        let severity = req.severity.unwrap_or(cur.severity);
        let status = req.status.unwrap_or(cur.status);
        let priority = req.priority.unwrap_or(cur.priority);
        let assignee = match req.assignee {
            Some(a) if a.is_empty() => None,
            Some(a) => Some(a),
            None => cur.assignee,
        };
        let tags = req.tags.unwrap_or(cur.tags);
        let resolution = match req.resolution {
            Some(r) if r.is_empty() => None,
            Some(r) => Some(r),
            None => cur.resolution,
        };
        let resolution_notes = match req.resolution_notes {
            Some(rn) if rn.is_empty() => None,
            Some(rn) => Some(rn),
            None => cur.resolution_notes,
        };
        let closed_at: Option<DateTime<Utc>> = if status == "closed" || status == "resolved" {
            cur.closed_at.or_else(|| Some(Utc::now()))
        } else {
            None
        };

        let did_acknowledge =
            prev_status == "new" && status != "new" && cur.acknowledged_at.is_none();
        let acknowledged_at = if did_acknowledge {
            Some(Utc::now())
        } else {
            cur.acknowledged_at
        };

        let runbook_url = match req.runbook_url {
            Some(ref r) if !r.trim().is_empty() => Some(r.trim().to_string()),
            Some(_) => cur.runbook_url.clone(),
            None => cur.runbook_url.clone(),
        };

        let mut updated = sqlx::query_as::<_, Case>(
            "UPDATE cases SET \
             title = $2, description = $3, severity = $4, status = $5, priority = $6, \
             assignee = $7, tags = $8, resolution = $9, resolution_notes = $10, \
             closed_at = $11, acknowledged_at = $12, runbook_url = $13, updated_at = now() \
             WHERE id = $1 \
             RETURNING id, case_number, title, description, severity, status, priority, \
                       assignee, tags, resolution, resolution_notes, source, \
                       created_at, updated_at, closed_at, acknowledged_at, due_at, runbook_url",
        )
        .bind(id)
        .bind(&title)
        .bind(&description)
        .bind(&severity)
        .bind(&status)
        .bind(priority)
        .bind(&assignee)
        .bind(&tags)
        .bind(&resolution)
        .bind(&resolution_notes)
        .bind(closed_at)
        .bind(acknowledged_at)
        .bind(&runbook_url)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)?;
        updated.apply_display_key();

        if did_acknowledge {
            let _ = self
                .add_timeline(
                    id,
                    "system",
                    "ack",
                    Some("Incident acknowledged (left new)"),
                    serde_json::json!({"from": prev_status, "to": updated.status}),
                )
                .await;
        }

        Ok(updated)
    }

    pub async fn add_timeline(
        &self,
        case_id: Uuid,
        actor: &str,
        entry_type: &str,
        body: Option<&str>,
        metadata: serde_json::Value,
    ) -> Result<TimelineEntry, StoreError> {
        let entry = sqlx::query_as::<_, TimelineEntry>(
            "INSERT INTO case_timeline (case_id, actor, entry_type, body, metadata) \
             VALUES ($1,$2,$3,$4,$5) \
             RETURNING id, case_id, actor, entry_type, body, metadata, created_at",
        )
        .bind(case_id)
        .bind(actor)
        .bind(entry_type)
        .bind(body)
        .bind(&metadata)
        .fetch_one(&self.pool)
        .await?;
        Ok(entry)
    }

    pub async fn list_timeline(&self, case_id: Uuid) -> Result<Vec<TimelineEntry>, StoreError> {
        let entries = sqlx::query_as::<_, TimelineEntry>(
            "SELECT id, case_id, actor, entry_type, body, metadata, created_at \
             FROM case_timeline WHERE case_id = $1 ORDER BY created_at ASC",
        )
        .bind(case_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(entries)
    }

    pub async fn link_event(
        &self,
        case_id: Uuid,
        event_id: Uuid,
        note: Option<&str>,
    ) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO case_linked_events (case_id, event_id, note) VALUES ($1,$2,$3) \
             ON CONFLICT (case_id, event_id) DO UPDATE SET \
             note = COALESCE(EXCLUDED.note, case_linked_events.note)",
        )
        .bind(case_id)
        .bind(event_id)
        .bind(note)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_linked_events(&self, case_id: Uuid) -> Result<Vec<LinkedEvent>, StoreError> {
        let events = sqlx::query_as::<_, LinkedEvent>(
            "SELECT event_id, note, linked_at \
             FROM case_linked_events WHERE case_id = $1 ORDER BY linked_at",
        )
        .bind(case_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(events)
    }

    pub async fn upsert_linked_alert(
        &self,
        case_id: Uuid,
        fp: &str,
        rule_id: Option<&str>,
        rule_title: Option<&str>,
        severity: Option<&str>,
        description: Option<&str>,
        seen_at: DateTime<Utc>,
        context: &serde_json::Value,
    ) -> Result<(), StoreError> {
        sqlx::query(
            "INSERT INTO case_linked_alerts \
             (case_id, fingerprint, rule_id, rule_title, severity, description, first_seen_at, last_seen_at, context) \
             VALUES ($1,$2,$3,$4,$5,$6,$7,$7,$8) \
             ON CONFLICT (case_id, fingerprint) DO UPDATE SET \
             last_seen_at = EXCLUDED.last_seen_at, \
             rule_id = COALESCE(EXCLUDED.rule_id, case_linked_alerts.rule_id), \
             rule_title = COALESCE(EXCLUDED.rule_title, case_linked_alerts.rule_title), \
             severity = COALESCE(EXCLUDED.severity, case_linked_alerts.severity), \
             description = COALESCE(NULLIF(EXCLUDED.description, ''), case_linked_alerts.description), \
             context = CASE \
               WHEN EXCLUDED.context = '{}'::jsonb THEN case_linked_alerts.context \
               ELSE EXCLUDED.context END",
        )
        .bind(case_id)
        .bind(fp)
        .bind(rule_id)
        .bind(rule_title)
        .bind(severity)
        .bind(description)
        .bind(seen_at)
        .bind(context)
        .execute(&self.pool)
        .await?;
        Ok(())
    }

    pub async fn list_linked_alerts(&self, case_id: Uuid) -> Result<Vec<LinkedAlert>, StoreError> {
        let alerts = sqlx::query_as::<_, LinkedAlert>(
            "SELECT fingerprint, rule_id, rule_title, severity, description, \
             first_seen_at, last_seen_at, context \
             FROM case_linked_alerts WHERE case_id = $1 ORDER BY first_seen_at",
        )
        .bind(case_id)
        .fetch_all(&self.pool)
        .await?;
        Ok(alerts)
    }

    pub async fn find_active_case_by_fingerprint(
        &self,
        fingerprint: &str,
    ) -> Result<Uuid, StoreError> {
        sqlx::query_scalar::<_, Uuid>(
            "SELECT c.id FROM cases c \
             INNER JOIN case_linked_alerts la ON la.case_id = c.id \
             WHERE la.fingerprint = $1 \
               AND c.status IN ('new','triaged','investigating','contained') \
             ORDER BY c.created_at DESC \
             LIMIT 1",
        )
        .bind(fingerprint)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)
    }

    pub async fn find_latest_case_by_fingerprint(
        &self,
        fingerprint: &str,
    ) -> Result<Uuid, StoreError> {
        sqlx::query_scalar::<_, Uuid>(
            "SELECT c.id FROM cases c \
             INNER JOIN case_linked_alerts la ON la.case_id = c.id \
             WHERE la.fingerprint = $1 \
             ORDER BY c.updated_at DESC \
             LIMIT 1",
        )
        .bind(fingerprint)
        .fetch_optional(&self.pool)
        .await?
        .ok_or(StoreError::NotFound)
    }

    pub async fn get_case_detail(&self, id: Uuid) -> Result<CaseDetail, StoreError> {
        let case = self.get_case(id).await?;
        let timeline = self.list_timeline(id).await?;
        let linked_alerts = self.list_linked_alerts(id).await?;
        let linked_events = self.list_linked_events(id).await?;
        Ok(CaseDetail {
            case,
            timeline,
            linked_alerts,
            linked_events,
        })
    }
}
