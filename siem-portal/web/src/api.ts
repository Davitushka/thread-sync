const ABSOLUTE_URL_RE = /^[a-z][a-z0-9+.-]*:\/\//i;
const RAW_ROUTER_BASE = (import.meta.env.BASE_URL as string | undefined)?.trim() || "/";

function normalizeRouterBase(base: string): string {
  const trimmed = base.trim();
  if (!trimmed || trimmed === "/") {
    return "/";
  }
  const withLeading = trimmed.startsWith("/") ? trimmed : `/${trimmed}`;
  return withLeading.replace(/\/+$/, "") || "/";
}

function defaultApiBaseForRouter(base: string): string {
  return base === "/" ? "/api/v1" : `${base}/api/v1`;
}

const DEFAULT_API_BASE = defaultApiBaseForRouter(normalizeRouterBase(RAW_ROUTER_BASE));
const RAW_API_BASE = (import.meta.env.VITE_API_BASE as string | undefined)?.trim() || DEFAULT_API_BASE;

function normalizeBasePath(base: string): string {
  if (ABSOLUTE_URL_RE.test(base)) {
    return base.replace(/\/+$/, "");
  }
  const trimmed = base.replace(/^\/+/, "").replace(/\/+$/, "");
  return trimmed ? `/${trimmed}` : DEFAULT_API_BASE;
}

function joinRequestPath(base: string, path: string): string {
  const suffix = path.startsWith("/") ? path : `/${path}`;
  return `${base}${suffix}`;
}

function normalizeRequestPath(path: string): string {
  if (ABSOLUTE_URL_RE.test(path)) {
    return path;
  }
  return path.startsWith("/") ? path : `/${path}`;
}

const API_BASE = normalizeBasePath(RAW_API_BASE);

const api = (path: string, init?: RequestInit) =>
  fetch(joinRequestPath(API_BASE, path), {
    ...init,
    headers: {
      "Content-Type": "application/json",
      ...(init?.headers || {}),
    },
  });

const suite = (path: string, init?: RequestInit) =>
  fetch(normalizeRequestPath(path), {
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
  /** Host-facing Vector HTTP ingest (`POST …/logs`). Newer portals only. */
  vector_http_base?: string;
  /** Redpanda admin HTTP API. Newer portals only. */
  redpanda_admin?: string;
};

export type UiConfig = {
  links: PortalLinks;
  suite?: {
    api_base: string;
    modules: string[];
    realtime?: {
      protocol: number;
      path: string;
    };
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

export type OperationsDashboard = {
  window_hours: number;
  step_sec: number;
  totals: {
    clickhouse_select_qps: number;
    clickhouse_insert_qps: number;
    redpanda_records_rate: number;
    vector_ingest_rate: number;
    vector_forward_rate: number;
    detection_processed_rate: number;
    firing_alerts: number;
    parser_in_flight: number;
    parse_errors_24h: number;
    dropped_alerts_24h: number;
    healthy_components: number;
    total_components: number;
  };
  component_status: Array<{ job: string; up: boolean; value: number }>;
  clickhouse_series: Array<{ ts: number; select_qps: number; insert_qps: number; failed_qps: number }>;
  vector_series: Array<{ ts: number; http_ingest_eps: number; to_redpanda_eps: number }>;
  pipeline_series: Array<{ ts: number; redpanda_records_eps: number; detection_processed_eps: number }>;
};

export type DataQualityDashboard = {
  window_hours: number;
  step_sec: number;
  lag_window_hours: number;
  kpis: {
    total_events: number;
    missing_source_ip_pct: number;
    p95_ingest_lag_ms: number;
    unique_source_types: number;
    parser_ok_rate: number;
    parser_error_rate: number;
    consumer_lag: number;
  };
  lag_series: Array<{ bucket: string; p95_lag_ms: number }>;
  parser_series: Array<{ ts: number; ok_rate: number; error_rate: number }>;
  consumer_lag_series: Array<{ ts: number; lag: number }>;
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

export type AlertsOverview = {
  totals: {
    total: number;
    active: number;
    critical: number;
    silenced: number;
    unique_sources: number;
  };
  severity_breakdown: Array<{ name: string; count: number }>;
  source_breakdown: Array<{ name: string; count: number }>;
  alerts: Array<{
    fingerprint: string;
    name: string;
    severity: string;
    state: string;
    source: string;
    summary: string;
    description: string;
    starts_at?: string;
    ends_at?: string;
    rule_id?: string;
    source_ip?: string;
    user_id?: string;
    silenced_count: number;
    labels: Record<string, string>;
    annotations: Record<string, string>;
  }>;
};

export type DetectionRow = {
  rule: string;
  severity: string;
  state: string;
  signal: string;
};

export type DetectionsOverview = {
  stats: {
    rules_count: number;
    pending_alerts: number;
    alert_capacity: number;
    firing_count: number;
    critical_firing: number;
  };
  severity_breakdown: Array<{ name: string; count: number }>;
  state_breakdown: Array<{ name: string; count: number }>;
  top_rules: Array<{ name: string; count: number }>;
  firing_rows: Array<{
    rule: string;
    severity: string;
    state: string;
    signal: string;
  }>;
  rules: Array<{
    id: string;
    title: string;
    severity: string;
    kind?: string;
    threshold?: number;
    window_sec?: number;
    firing_count: number;
  }>;
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

export async function getOperationsDashboard(hours = 24): Promise<OperationsDashboard> {
  const r = await api(`/operations?hours=${hours}`);
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}

export async function getDataQualityDashboard(hours = 24): Promise<DataQualityDashboard> {
  const r = await api(`/data-quality?hours=${hours}`);
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

export async function getAlertsOverview(): Promise<AlertsOverview> {
  const r = await api("/alerts/overview");
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

export async function getDetectionsOverview(): Promise<DetectionsOverview> {
  const r = await api("/detections/overview");
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}

export async function searchEvents(params: Record<string, string>): Promise<EventSearchResponse> {
  const q = new URLSearchParams(params);
  const r = await api(`/events/search?${q.toString()}`);
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}

export async function getEvent(id: string): Promise<EventDetail> {
  const r = await api(`/events/${id}`);
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}

export async function getEntityContext(kind: string, value: string): Promise<EntityContext> {
  const r = await api(`/entities/${encodeURIComponent(kind)}/${encodeURIComponent(value)}/context`);
  if (!r.ok) throw new Error(await r.text());
  return r.json();
}
