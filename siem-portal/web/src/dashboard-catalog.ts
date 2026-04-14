export type NativeDashboardKind = "native";
export type GrafanaDashboardKind = "grafana";

export type DashboardEntry = {
  id: string;
  group: "SOC Core" | "Platform" | "Deep Dive";
  title: string;
  description: string;
  kind: NativeDashboardKind | GrafanaDashboardKind;
  path?: string;
  uid?: string;
  hours?: number;
  status: "native" | "hybrid" | "grafana";
  badge: string;
  spotlight?: string;
  priority: "daily" | "support" | "bridge" | "deep-dive";
  audience: "soc" | "platform" | "engineering";
};

export const DASHBOARD_GROUPS = ["SOC Core", "Platform", "Deep Dive"] as const;

export const DASHBOARD_TIME_RANGES = [
  { value: "now-6h", label: "Last 6h" },
  { value: "now-24h", label: "Last 24h" },
  { value: "now-7d", label: "Last 7d" },
  { value: "now-30d", label: "Last 30d" },
] as const;

export const DASHBOARDS: DashboardEntry[] = [
  {
    id: "overview",
    group: "SOC Core",
    title: "SOC Overview",
    description: "Main SOC overview with events, severity mix, top source IPs and recent signal feed.",
    kind: "native",
    path: "/",
    hours: 24,
    status: "native",
    badge: "Daily cockpit",
    spotlight: "Native",
    priority: "daily",
    audience: "soc",
  },
  {
    id: "detections",
    group: "SOC Core",
    title: "Detections Console",
    description: "Firing rules, rule pressure and detection-engine health in the daily analyst loop.",
    kind: "native",
    path: "/detections",
    status: "native",
    badge: "Engine operations",
    spotlight: "Native",
    priority: "daily",
    audience: "soc",
  },
  {
    id: "alerts",
    group: "SOC Core",
    title: "Alerts Console",
    description: "Operational queue for alert triage, ownership, suppression signals and case pivots.",
    kind: "native",
    path: "/alerts",
    status: "native",
    badge: "Triage queue",
    spotlight: "Native",
    priority: "daily",
    audience: "soc",
  },
  {
    id: "events",
    group: "SOC Core",
    title: "Log Explorer",
    description: "Native event and log explorer for hunting, pivots, timeline context and case promotion.",
    kind: "native",
    path: "/events",
    status: "native",
    badge: "Log explorer",
    spotlight: "Native",
    priority: "daily",
    audience: "soc",
  },
  {
    id: "workbench",
    group: "SOC Core",
    title: "Case Workbench",
    description: "Case queue and investigation workflows for analyst-facing deep-dive work.",
    kind: "native",
    path: "/cases",
    status: "native",
    badge: "Investigation flow",
    priority: "support",
    audience: "soc",
  },
  {
    id: "infrastructure",
    group: "Platform",
    title: "Infrastructure",
    description: "CPU, memory, disk, network, containers and general platform health.",
    kind: "native",
    path: "/infrastructure",
    hours: 6,
    status: "native",
    badge: "Platform health",
    spotlight: "Native",
    priority: "support",
    audience: "platform",
  },
  {
    id: "operations",
    group: "Platform",
    title: "Operations",
    description: "Vector, parser, Redpanda, ClickHouse, Grafana and Alertmanager flow validation.",
    kind: "native",
    path: "/operations",
    hours: 24,
    status: "native",
    badge: "Pipeline center",
    spotlight: "Native",
    priority: "support",
    audience: "platform",
  },
  {
    id: "validation",
    group: "Platform",
    title: "Validation",
    description: "Native stack validation workspace for service reachability, ingest continuity, parser quality and dashboard trust checks.",
    kind: "native",
    path: "/validation",
    status: "native",
    badge: "Validation",
    spotlight: "Native",
    priority: "support",
    audience: "platform",
  },
  {
    id: "data-quality",
    group: "Platform",
    title: "Data Quality",
    description: "Parser success/error, ingest lag and consumer lag for the data pipeline.",
    kind: "native",
    path: "/data-quality",
    hours: 24,
    status: "native",
    badge: "Trust layer",
    spotlight: "Native",
    priority: "support",
    audience: "platform",
  },
  {
    id: "clickhouse-data",
    group: "Deep Dive",
    title: "ClickHouse Data Analysis",
    description: "SQL analysis of `siem.events` and `system.query_log` inside Grafana dashboards.",
    kind: "grafana",
    uid: "ch-data-analysis-sql",
    status: "grafana",
    badge: "SQL deep dive",
    priority: "deep-dive",
    audience: "engineering",
  },
  {
    id: "clickhouse-query",
    group: "Deep Dive",
    title: "ClickHouse Query Analysis",
    description: "Slow queries, heavy statements and query profile details for ClickHouse.",
    kind: "grafana",
    uid: "ch-query-analysis-sql",
    status: "grafana",
    badge: "Query profiling",
    priority: "deep-dive",
    audience: "engineering",
  },
  {
    id: "correlator",
    group: "Deep Dive",
    title: "Correlator Metrics",
    description: "Deep technical metrics for the correlator engine and firing alert throughput.",
    kind: "grafana",
    uid: "siem-correlator-metrics",
    status: "grafana",
    badge: "Engine deep dive",
    priority: "deep-dive",
    audience: "engineering",
  },
  {
    id: "prometheus",
    group: "Deep Dive",
    title: "Prometheus Stats",
    description: "Scrape health, sample volume and Prometheus internal metrics.",
    kind: "grafana",
    uid: "siem-prometheus-stats",
    status: "grafana",
    badge: "Observability backend",
    priority: "deep-dive",
    audience: "engineering",
  },
];

export function grafanaDashboardUrl(root: string, uid: string, from: string, embedded: boolean): string {
  const base = root.replace(/\/$/, "");
  const params = new URLSearchParams({
    orgId: "1",
    theme: "dark",
    from,
    to: "now",
  });
  if (embedded) {
    params.set("kiosk", "tv");
  }
  return `${base}/d/${uid}?${params.toString()}`;
}
