const api = (path: string, init?: RequestInit) =>
  fetch(`/api/v1${path}`, {
    ...init,
    headers: {
      "Content-Type": "application/json",
      ...(init?.headers || {}),
    },
  });

const suite = (path: string, init?: RequestInit) =>
  fetch(path, {
    ...init,
    headers: {
      "Content-Type": "application/json",
      ...(init?.headers || {}),
    },
  });

export function actorHeader(name: string): HeadersInit {
  return { "X-SOC-Actor": name || "analyst" };
}

export type PortalLinks = {
  grafana: string;
  prometheus: string;
  alertmanager: string;
  case_management: string;
  siem_overview_dashboard: string;
};

export type UiConfig = {
  links: PortalLinks;
  suite?: {
    api_base: string;
    modules: string[];
  };
};

export type StackComponent = {
  ok?: boolean;
  latency_ms?: number;
  status?: number;
  error?: string;
  detail?: unknown;
};

export type StackStatus = {
  elapsed_ms: number;
  components: Record<string, StackComponent>;
};

export type OverviewDashboard = {
  window_hours: number;
  bucket_minutes: number;
  kpis: {
    total_events_24h: number;
    critical_events_24h: number;
    error_pct_24h: number;
  };
  events_per_minute: Array<{
    minute: string;
    events: number;
  }>;
  severity_breakdown: Array<{
    severity: string;
    events: number;
  }>;
  severity_timeline: Array<{
    bucket: string;
    critical: number;
    error: number;
    warning: number;
  }>;
  source_breakdown: Array<{
    source_type: string;
    events: number;
  }>;
  top_source_ips: Array<{
    source_ip: string;
    events: number;
    threats: number;
  }>;
  recent_security_events: Array<{
    timestamp: string;
    event_id: string;
    source_type: string;
    severity: string;
    host: string;
    source_ip?: string;
    message: string;
  }>;
};

export type InfrastructureDashboard = {
  window_hours: number;
  step_sec: number;
  host: {
    cpu_usage_pct: number;
    memory_usage_pct: number;
    disk_usage_pct: number;
    network_rx_bps: number;
    network_tx_bps: number;
    uptime_sec: number;
    container_count: number;
    total_container_cpu_pct: number;
    total_container_memory_bytes: number;
    healthy_components: number;
    total_components: number;
  };
  cpu_series: Array<{ ts: number; value: number }>;
  network_rx_series: Array<{ ts: number; value: number }>;
  network_tx_series: Array<{ ts: number; value: number }>;
  top_cpu_containers: Array<{ name: string; value: number }>;
  top_memory_containers: Array<{ name: string; value: number }>;
  component_status: Array<{ job: string; up: boolean; value: number }>;
};

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
  context?: Record<string, unknown>;
};

export type LinkedEvent = {
  event_id: string;
  note?: string;
  linked_at: string;
};

export type CaseDetail = Case & {
  timeline: TimelineEntry[];
  linked_alerts: LinkedAlert[];
  linked_events: LinkedEvent[];
};

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

export type AlertItem = {
  fingerprint: string;
  status: {
    state?: string;
    silencedBy?: string[];
  };
  labels: {
    alertname?: string;
    severity?: string;
    instance?: string;
    job?: string;
    rule_id?: string;
    source_ip?: string;
    user_id?: string;
  };
  annotations?: {
    description?: string;
    summary?: string;
  };
  startsAt?: string;
  endsAt?: string;
};

export type DetectionRow = {
  rule: string;
  severity: string;
  state: string;
  signal: string;
};

export type CorrelatorStats = {
  rules_count: number;
  pending_alerts: number;
  alert_capacity: number;
  timestamp: string;
};

export type RuleCard = {
  id: string;
  title: string;
  severity: string;
  kind?: string;
  threshold?: number;
  window_sec?: number;
};

export type EventRow = {
  timestamp: string;
  event_id: string;
  source_type: string;
  event_type: string;
  severity: string;
  host: string;
  source_ip?: string;
  user_id?: string;
  action?: string;
  status_code?: number;
  url_path?: string;
  message: string;
};

export type EventSearchResponse = {
  rows: EventRow[];
  meta: {
    limit: number;
    returned: number;
    filters: {
      start: string;
      end: string;
      severity?: string;
      source_type?: string;
      host?: string;
      source_ip?: string;
      user_id?: string;
      q?: string;
    };
  };
};

export type EventDetail = {
  event: EventRow;
  duration_ms?: number;
  http_method?: string;
  metadata: Record<string, unknown>;
  agent_version: string;
  ingest_ts: string;
  enrich: {
    geo_country_iso?: string;
    geo_country_name?: string;
    geo_city?: string;
    geo_lat?: number;
    geo_lon?: number;
    geo_asn?: number;
    geo_org?: string;
  };
};

export type EntityContext = {
  entity: {
    kind: string;
    value: string;
  };
  recent_events: EventRow[];
  metrics: {
    total_events_24h: number;
    error_events_24h: number;
    top_hosts: string[];
  };
};

type PromQueryResponse = {
  data?: {
    result?: Array<{
      metric?: Record<string, string>;
      value?: [number, string];
    }>;
  };
};

export async function uiConfig(): Promise<UiConfig> {
  const r = await api("/ui/config");
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}

export async function stackStatus(): Promise<StackStatus> {
  const r = await api("/stack/status");
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}

export async function getOverviewDashboard(hours = 24): Promise<OverviewDashboard> {
  const r = await api(`/overview?hours=${hours}`);
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}

export async function getInfrastructureDashboard(hours = 6): Promise<InfrastructureDashboard> {
  const r = await api(`/infrastructure?hours=${hours}`);
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}

export async function listCases(params: Record<string, string>): Promise<{ cases: Case[]; total: number }> {
  const q = new URLSearchParams(params);
  const r = await api(`/proxy/cases?${q}`);
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}

export async function getCase(id: string): Promise<CaseDetail> {
  const r = await api(`/proxy/cases/${id}`);
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}

export async function getInvestigation(id: string): Promise<Investigation> {
  const r = await api(`/proxy/cases/${id}/investigate`);
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
  const r = await api("/proxy/cases", {
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
      source: "suite",
    }),
  });
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}

export async function patchCase(id: string, patch: Record<string, unknown>, actor: string): Promise<Case> {
  const r = await api(`/proxy/cases/${id}`, {
    method: "PATCH",
    headers: actorHeader(actor),
    body: JSON.stringify(patch),
  });
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}

export async function addComment(id: string, body: string, actor: string): Promise<TimelineEntry> {
  const r = await api(`/proxy/cases/${id}/timeline`, {
    method: "POST",
    headers: actorHeader(actor),
    body: JSON.stringify({ body }),
  });
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}

export async function linkEvent(id: string, eventId: string, note: string | undefined, actor: string) {
  const r = await api(`/proxy/cases/${id}/events`, {
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
  extra: { rule_id?: string; rule_title?: string; severity?: string; description?: string; context?: unknown },
  actor: string
) {
  const r = await api(`/proxy/cases/${id}/alerts`, {
    method: "POST",
    headers: actorHeader(actor),
    body: JSON.stringify({ fingerprint, ...extra }),
  });
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}

export async function getAlerts(): Promise<AlertItem[]> {
  const r = await api("/proxy/alertmanager/v2/alerts");
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}

export async function getCorrelatorStats(): Promise<CorrelatorStats> {
  const r = await api("/proxy/correlator/stats");
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}

export async function getCorrelatorRules(): Promise<RuleCard[]> {
  const r = await api("/proxy/correlator/rules");
  if (!r.ok) throw new Error(await r.text());
  const data = await r.json();
  return Array.isArray(data) ? data : Array.isArray(data.rules) ? data.rules : [];
}

export async function getPromAlerts(): Promise<DetectionRow[]> {
  const q = encodeURIComponent("ALERTS");
  const r = await api(`/proxy/prometheus/query?query=${q}`);
  if (!r.ok) throw new Error(await r.text());
  const data: PromQueryResponse = await r.json();
  return (data.data?.result ?? []).map((item) => ({
    rule: item.metric?.alertname ?? "alert",
    severity: item.metric?.severity ?? "unknown",
    state: item.metric?.alertstate ?? "firing",
    signal: item.value?.[1] ?? "0",
  }));
}

export async function searchEvents(params: Record<string, string>): Promise<EventSearchResponse> {
  const q = new URLSearchParams(params);
  const r = await suite(`/api/v1/events/search?${q.toString()}`);
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}

export async function getEvent(id: string): Promise<EventDetail> {
  const r = await suite(`/api/v1/events/${id}`);
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}

export async function getEntityContext(kind: string, value: string): Promise<EntityContext> {
  const r = await suite(`/api/v1/entities/${encodeURIComponent(kind)}/${encodeURIComponent(value)}/context`);
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}
