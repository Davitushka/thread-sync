import { matchPath } from "react-router-dom";

export type HeaderMeta = {
  title: string;
  subtitle: string;
  crumbs: Array<{ label: string; to?: string }>;
  mode?: string;
};

export const DEFAULT_HEADER_META: HeaderMeta = {
  title: "Unified Analyst Suite",
  subtitle: "One entry point for monitoring, triage, investigation and casework.",
  crumbs: [{ label: "Suite" }],
  mode: "suite",
};

export const SUITE_NAV_ITEMS = [
  {
    to: "/",
    label: "Overview",
    description: "Signals, KPIs and quick pivots for the daily SOC loop.",
    end: true,
  },
  {
    to: "/infrastructure",
    label: "Infrastructure",
    description: "Host, network, container and platform health signals.",
    end: false,
  },
  {
    to: "/operations",
    label: "Operations",
    description: "Pipeline flow, service uptime and storage throughput in a native operations center.",
    end: false,
  },
  {
    to: "/data-quality",
    label: "Data Quality",
    description: "Trust layer for event completeness, parser quality and ingest lag.",
    end: false,
  },
  {
    to: "/dashboards",
    label: "Dashboards",
    description: "Native analytics hub with Grafana reserved for deep dives.",
    end: false,
  },
  {
    to: "/alerts",
    label: "Alerts",
    description: "Dense triage inbox for the active alert queue.",
    end: false,
  },
  {
    to: "/detections",
    label: "Detections",
    description: "Engine health, noisy rules and firing signals.",
    end: false,
  },
  {
    to: "/events",
    label: "Events",
    description: "Native event search, pivots and entity context.",
    end: false,
  },
  {
    to: "/cases",
    label: "Cases",
    description: "Operational case workflow with ownership and investigation pivots.",
    end: false,
  },
] as const;

export const ROUTE_META: Array<{ path: string; end: boolean; meta: HeaderMeta }> = [
  {
    path: "/",
    end: true,
    meta: {
      title: "SOC overview",
      subtitle: "Key signals, stack health and fast pivots for the daily analyst loop.",
      crumbs: [{ label: "Overview" }],
      mode: "suite",
    },
  },
  {
    path: "/infrastructure",
    end: true,
    meta: {
      title: "Infrastructure",
      subtitle: "Host, network, container and health signals across the platform.",
      crumbs: [{ label: "Infrastructure" }],
      mode: "ops",
    },
  },
  {
    path: "/operations",
    end: true,
    meta: {
      title: "Operations center",
      subtitle: "Native pipeline monitoring for service status, ClickHouse workload and flow pressure.",
      crumbs: [{ label: "Operations" }],
      mode: "ops",
    },
  },
  {
    path: "/data-quality",
    end: true,
    meta: {
      title: "Data quality",
      subtitle: "Trust layer for completeness, parser quality, ingest lag and consumer delay.",
      crumbs: [{ label: "Data Quality" }],
      mode: "trust",
    },
  },
  {
    path: "/dashboards",
    end: true,
    meta: {
      title: "Dashboards hub",
      subtitle: "Native daily workspaces first, Grafana reserved for advanced deep-dive analysis.",
      crumbs: [{ label: "Dashboards" }],
      mode: "analytics",
    },
  },
  {
    path: "/alerts",
    end: true,
    meta: {
      title: "Alerts console",
      subtitle: "Dense triage inbox for alerts, queues and decisive follow-up actions.",
      crumbs: [{ label: "Alerts" }],
      mode: "triage",
    },
  },
  {
    path: "/detections",
    end: true,
    meta: {
      title: "Detections console",
      subtitle: "Engine health, firing rules and noisy signals in one place.",
      crumbs: [{ label: "Detections" }],
      mode: "triage",
    },
  },
  {
    path: "/events",
    end: true,
    meta: {
      title: "Event search",
      subtitle: "Native ClickHouse search, pivots and structured event detail inside the suite.",
      crumbs: [{ label: "Events" }],
      mode: "hunt",
    },
  },
  {
    path: "/cases",
    end: true,
    meta: {
      title: "Cases",
      subtitle: "Unified case workflow on top of the portal BFF and case-management services.",
      crumbs: [{ label: "Cases" }],
      mode: "casework",
    },
  },
  {
    path: "/cases/:id",
    end: true,
    meta: {
      title: "Case detail",
      subtitle: "Manage status, timeline, linked signals and analyst ownership.",
      crumbs: [{ label: "Cases", to: "/cases" }, { label: "Case detail" }],
      mode: "casework",
    },
  },
  {
    path: "/cases/:id/investigate",
    end: true,
    meta: {
      title: "Investigation workbench",
      subtitle: "Case summary, merged feed and investigative pivots in one workspace.",
      crumbs: [{ label: "Cases", to: "/cases" }, { label: "Investigation" }],
      mode: "investigation",
    },
  },
];

export function resolveHeaderMeta(pathname: string): HeaderMeta {
  return (
    ROUTE_META.find((entry) => matchPath({ path: entry.path, end: entry.end }, pathname))?.meta ??
    DEFAULT_HEADER_META
  );
}
