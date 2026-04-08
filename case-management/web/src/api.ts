const api = (path: string, init?: RequestInit) =>
  fetch(`/api/v1${path}`, {
    ...init,
    headers: {
      "Content-Type": "application/json",
      ...(init?.headers || {}),
    },
  });

export function actorHeader(name: string): HeadersInit {
  return { "X-SOC-Actor": name || "analyst" };
}

export type Case = {
  id: string;
  case_number: number;
  display_key: string;
  title: string;
  description: string;
  severity: string;
  status: string;
  priority: number;
  assignee?: string;
  tags: string[];
  resolution?: string;
  resolution_notes?: string;
  source: string;
  created_at: string;
  updated_at: string;
  closed_at?: string;
  acknowledged_at?: string;
  due_at?: string;
  runbook_url?: string;
};

export type CaseDetail = Case & {
  timeline: TimelineEntry[];
  linked_alerts: LinkedAlert[];
  linked_events: LinkedEvent[];
};

export type TimelineEntry = {
  id: string;
  case_id: string;
  actor: string;
  entry_type: string;
  body?: string;
  metadata: unknown;
  created_at: string;
};

export type LinkedAlert = {
  fingerprint: string;
  rule_id?: string;
  rule_title?: string;
  severity?: string;
  description?: string;
  first_seen_at: string;
  last_seen_at: string;
  /** Снимок labels Alertmanager (source_ip, …) для pivot в ClickHouse */
  context?: Record<string, unknown>;
};

export type LinkedEvent = {
  event_id: string;
  note?: string;
  linked_at: string;
};

export async function listCases(params: Record<string, string>): Promise<{ cases: Case[]; total: number }> {
  const q = new URLSearchParams(params);
  const r = await api(`/cases?${q}`);
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}

export async function getCase(id: string): Promise<CaseDetail> {
  const r = await api(`/cases/${id}`);
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}

/** Сводка расследования: ссылки Grafana + предложенные запросы ClickHouse (сервер не меняет БД). */
export type Investigation = {
  case_id: string;
  display_key: string;
  due_at?: string;
  acknowledged_at?: string;
  runbook_url?: string;
  grafana: {
    siem_overview?: string;
    explore_clickhouse_preset?: string;
    explore_loki?: string;
    data_quality_dashboard?: string;
  };
  suggested_clickhouse_queries: Array<{ title: string; sql: string }>;
  process: { status_workflow: string[]; sla_hint: string };
};

export async function getInvestigation(id: string): Promise<Investigation> {
  const r = await api(`/cases/${id}/investigate`);
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}

export async function createCase(
  body: {
    title: string;
    description?: string;
    severity?: string;
    status?: string;
    priority?: number;
    assignee?: string;
    tags?: string[];
  },
  actor: string
): Promise<Case> {
  const r = await api("/cases", {
    method: "POST",
    headers: actorHeader(actor),
    body: JSON.stringify({
      title: body.title,
      description: body.description ?? "",
      severity: body.severity ?? "medium",
      status: body.status ?? "new",
      priority: body.priority ?? 2,
      assignee: body.assignee || undefined,
      tags: body.tags ?? [],
      source: "api",
    }),
  });
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}

export async function patchCase(id: string, patch: Record<string, unknown>, actor: string): Promise<Case> {
  const r = await api(`/cases/${id}`, {
    method: "PATCH",
    headers: actorHeader(actor),
    body: JSON.stringify(patch),
  });
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}

export async function addComment(id: string, body: string, actor: string): Promise<TimelineEntry> {
  const r = await api(`/cases/${id}/timeline`, {
    method: "POST",
    headers: actorHeader(actor),
    body: JSON.stringify({ body }),
  });
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}

export async function linkEvent(id: string, eventId: string, note: string | undefined, actor: string) {
  const r = await api(`/cases/${id}/events`, {
    method: "POST",
    headers: actorHeader(actor),
    body: JSON.stringify({ event_id: eventId, note: note || undefined }),
  });
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}

export async function linkAlert(
  id: string,
  fingerprint: string,
  extra: { rule_id?: string; rule_title?: string; severity?: string; description?: string },
  actor: string
) {
  const r = await api(`/cases/${id}/alerts`, {
    method: "POST",
    headers: actorHeader(actor),
    body: JSON.stringify({ fingerprint, ...extra }),
  });
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}
