import { matchPath } from "react-router-dom";

export type HeaderMeta = {
  title: string;
  subtitle: string;
  crumbs: Array<{ label: string; to?: string }>;
  mode?: string;
};

export type ShellGroupId = "mission-control" | "detection-triage" | "response" | "platform";
export type WorkspaceKind = "workspace" | "document";

export type WorkspaceMetaOverride = Partial<
  HeaderMeta & {
    label: string;
    tabLabel: string;
    description: string;
    iconKey: string;
    closable: boolean;
    keywords: string;
  }
>;

export type SuiteRouteDefinition = {
  id: string;
  path: string;
  to: string;
  label: string;
  tabLabel: string;
  description: string;
  end: boolean;
  nav: boolean;
  navId: string;
  groupId: ShellGroupId;
  iconKey: string;
  workspaceKind: WorkspaceKind;
  closable: boolean;
  defaultOpen: boolean;
  defaultPinned: boolean;
  keywords: string;
  meta: HeaderMeta;
};

export type SuiteNavItem = Pick<
  SuiteRouteDefinition,
  "id" | "to" | "label" | "description" | "end" | "groupId" | "iconKey" | "tabLabel" | "defaultPinned" | "defaultOpen" | "keywords"
>;

export type SuiteNavGroup = {
  id: ShellGroupId;
  label: string;
  description: string;
  defaultOpen: boolean;
  items: SuiteNavItem[];
};

export type ResolvedWorkspaceMeta = {
  id: string;
  path: string;
  navId: string | null;
  label: string;
  tabLabel: string;
  description: string;
  iconKey: string;
  groupId: ShellGroupId;
  workspaceKind: WorkspaceKind;
  closable: boolean;
  defaultOpen: boolean;
  defaultPinned: boolean;
  keywords: string;
  title: string;
  subtitle: string;
  crumbs: Array<{ label: string; to?: string }>;
  mode?: string;
};

export const DEFAULT_HEADER_META: HeaderMeta = {
  title: "Unified analyst console",
  subtitle: "Native command surface for monitoring, triage, hunting and response workflows.",
  crumbs: [{ label: "Suite" }],
  mode: "suite",
};

const GROUP_DEFS: Array<{
  id: ShellGroupId;
  label: string;
  description: string;
  defaultOpen: boolean;
}> = [
  {
    id: "mission-control",
    label: "Mission Control",
    description: "Overview dashboards and command spaces for daily posture monitoring.",
    defaultOpen: true,
  },
  {
    id: "detection-triage",
    label: "Detection & Triage",
    description: "Alert queues, detection pressure and raw event pivots for active response.",
    defaultOpen: true,
  },
  {
    id: "response",
    label: "Response",
    description: "Case queues, investigation workbenches and response ownership.",
    defaultOpen: true,
  },
  {
    id: "platform",
    label: "Platform",
    description: "Infrastructure, pipeline flow and data trust views for the SIEM stack.",
    defaultOpen: false,
  },
];

export const SUITE_ROUTE_REGISTRY: SuiteRouteDefinition[] = [
  {
    id: "overview",
    path: "/",
    to: "/",
    label: "Overview",
    tabLabel: "Overview",
    description: "Signals, KPIs and quick pivots for the daily SOC loop.",
    end: true,
    nav: true,
    navId: "overview",
    groupId: "mission-control",
    iconKey: "overview",
    workspaceKind: "workspace",
    closable: false,
    defaultOpen: true,
    defaultPinned: true,
    keywords: "overview mission control kpi posture suite",
    meta: {
      title: "SOC overview",
      subtitle: "Key signals, stack health and fast pivots for the daily analyst loop.",
      crumbs: [{ label: "Mission Control" }, { label: "Overview" }],
      mode: "suite",
    },
  },
  {
    id: "dashboards",
    path: "/dashboards",
    to: "/dashboards",
    label: "Dashboards",
    tabLabel: "Dashboards",
    description: "Native analytics hub with Grafana reserved for deep dives.",
    end: true,
    nav: true,
    navId: "dashboards",
    groupId: "mission-control",
    iconKey: "dashboards",
    workspaceKind: "workspace",
    closable: true,
    defaultOpen: false,
    defaultPinned: false,
    keywords: "dashboards analytics grafana deep dive",
    meta: {
      title: "Dashboards hub",
      subtitle: "Native daily workspaces first, Grafana reserved for advanced deep-dive analysis.",
      crumbs: [{ label: "Mission Control" }, { label: "Dashboards" }],
      mode: "analytics",
    },
  },
  {
    id: "alerts",
    path: "/alerts",
    to: "/alerts",
    label: "Alerts",
    tabLabel: "Alerts",
    description: "Dense triage inbox for the active alert queue.",
    end: true,
    nav: true,
    navId: "alerts",
    groupId: "detection-triage",
    iconKey: "alerts",
    workspaceKind: "workspace",
    closable: true,
    defaultOpen: true,
    defaultPinned: true,
    keywords: "alerts triage inbox alertmanager queue",
    meta: {
      title: "Alerts console",
      subtitle: "Dense triage inbox for alerts, queues and decisive follow-up actions.",
      crumbs: [{ label: "Detection & Triage" }, { label: "Alerts" }],
      mode: "triage",
    },
  },
  {
    id: "detections",
    path: "/detections",
    to: "/detections",
    label: "Detections",
    tabLabel: "Detections",
    description: "Engine health, noisy rules and firing signals.",
    end: true,
    nav: true,
    navId: "detections",
    groupId: "detection-triage",
    iconKey: "detections",
    workspaceKind: "workspace",
    closable: true,
    defaultOpen: false,
    defaultPinned: false,
    keywords: "detections correlator rules firing noisy",
    meta: {
      title: "Detections console",
      subtitle: "Engine health, firing rules and noisy signals in one place.",
      crumbs: [{ label: "Detection & Triage" }, { label: "Detections" }],
      mode: "triage",
    },
  },
  {
    id: "events",
    path: "/events",
    to: "/events",
    label: "Events",
    tabLabel: "Events",
    description: "Native event search, pivots and entity context.",
    end: true,
    nav: true,
    navId: "events",
    groupId: "detection-triage",
    iconKey: "events",
    workspaceKind: "workspace",
    closable: true,
    defaultOpen: false,
    defaultPinned: false,
    keywords: "events search hunt clickhouse entity",
    meta: {
      title: "Event search",
      subtitle: "Native ClickHouse search, pivots and structured event detail inside the suite.",
      crumbs: [{ label: "Detection & Triage" }, { label: "Events" }],
      mode: "hunt",
    },
  },
  {
    id: "cases",
    path: "/cases",
    to: "/cases",
    label: "Cases",
    tabLabel: "Cases",
    description: "Operational case workflow with ownership and investigation pivots.",
    end: true,
    nav: true,
    navId: "cases",
    groupId: "response",
    iconKey: "cases",
    workspaceKind: "workspace",
    closable: true,
    defaultOpen: true,
    defaultPinned: true,
    keywords: "cases response investigation ownership",
    meta: {
      title: "Cases",
      subtitle: "Unified case workflow on top of the portal BFF and case-management services.",
      crumbs: [{ label: "Response" }, { label: "Cases" }],
      mode: "casework",
    },
  },
  {
    id: "case-detail",
    path: "/cases/:id",
    to: "/cases",
    label: "Case detail",
    tabLabel: "Case",
    description: "Manage status, timeline, linked signals and analyst ownership.",
    end: true,
    nav: false,
    navId: "cases",
    groupId: "response",
    iconKey: "case-detail",
    workspaceKind: "document",
    closable: true,
    defaultOpen: false,
    defaultPinned: false,
    keywords: "case detail timeline linked alerts events",
    meta: {
      title: "Case detail",
      subtitle: "Manage status, timeline, linked signals and analyst ownership.",
      crumbs: [{ label: "Response" }, { label: "Cases", to: "/cases" }, { label: "Case detail" }],
      mode: "casework",
    },
  },
  {
    id: "investigation",
    path: "/cases/:id/investigate",
    to: "/cases",
    label: "Investigation",
    tabLabel: "Investigation",
    description: "Case summary, merged feed and investigative pivots in one workspace.",
    end: true,
    nav: false,
    navId: "cases",
    groupId: "response",
    iconKey: "investigation",
    workspaceKind: "document",
    closable: true,
    defaultOpen: false,
    defaultPinned: false,
    keywords: "investigation workbench response case",
    meta: {
      title: "Investigation workbench",
      subtitle: "Case summary, merged feed and investigative pivots in one workspace.",
      crumbs: [{ label: "Response" }, { label: "Cases", to: "/cases" }, { label: "Investigation" }],
      mode: "investigation",
    },
  },
  {
    id: "infrastructure",
    path: "/infrastructure",
    to: "/infrastructure",
    label: "Infrastructure",
    tabLabel: "Infrastructure",
    description: "Host, network, container and platform health signals.",
    end: true,
    nav: true,
    navId: "infrastructure",
    groupId: "platform",
    iconKey: "infrastructure",
    workspaceKind: "workspace",
    closable: true,
    defaultOpen: false,
    defaultPinned: false,
    keywords: "infrastructure hosts containers platform health",
    meta: {
      title: "Infrastructure",
      subtitle: "Host, network, container and health signals across the platform.",
      crumbs: [{ label: "Platform" }, { label: "Infrastructure" }],
      mode: "ops",
    },
  },
  {
    id: "operations",
    path: "/operations",
    to: "/operations",
    label: "Operations",
    tabLabel: "Operations",
    description: "Pipeline flow, service uptime and storage throughput in a native operations center.",
    end: true,
    nav: true,
    navId: "operations",
    groupId: "platform",
    iconKey: "operations",
    workspaceKind: "workspace",
    closable: true,
    defaultOpen: false,
    defaultPinned: false,
    keywords: "operations pipeline throughput clickhouse vector",
    meta: {
      title: "Operations center",
      subtitle: "Native pipeline monitoring for service status, ClickHouse workload and flow pressure.",
      crumbs: [{ label: "Platform" }, { label: "Operations" }],
      mode: "ops",
    },
  },
  {
    id: "data-quality",
    path: "/data-quality",
    to: "/data-quality",
    label: "Data Quality",
    tabLabel: "Data Quality",
    description: "Trust layer for event completeness, parser quality and ingest lag.",
    end: true,
    nav: true,
    navId: "data-quality",
    groupId: "platform",
    iconKey: "data-quality",
    workspaceKind: "workspace",
    closable: true,
    defaultOpen: false,
    defaultPinned: false,
    keywords: "data quality trust completeness ingest lag",
    meta: {
      title: "Data quality",
      subtitle: "Trust layer for completeness, parser quality, ingest lag and consumer delay.",
      crumbs: [{ label: "Platform" }, { label: "Data Quality" }],
      mode: "trust",
    },
  },
  {
    id: "validation",
    path: "/validation",
    to: "/validation",
    label: "Validation",
    tabLabel: "Validation",
    description: "Native trust and health checks for service reachability, ingest continuity and parser quality.",
    end: true,
    nav: true,
    navId: "validation",
    groupId: "platform",
    iconKey: "data-quality",
    workspaceKind: "workspace",
    closable: true,
    defaultOpen: false,
    defaultPinned: false,
    keywords: "validation trust checks ingest continuity parser service health",
    meta: {
      title: "Validation workspace",
      subtitle: "Native replacement for stack validation checks, trust signals and dashboard readiness.",
      crumbs: [{ label: "Platform" }, { label: "Validation" }],
      mode: "trust",
    },
  },
];

export const SUITE_NAV_ITEMS: SuiteNavItem[] = SUITE_ROUTE_REGISTRY.filter((route) => route.nav).map((route) => ({
  id: route.id,
  to: route.to,
  label: route.label,
  description: route.description,
  end: route.end,
  groupId: route.groupId,
  iconKey: route.iconKey,
  tabLabel: route.tabLabel,
  defaultPinned: route.defaultPinned,
  defaultOpen: route.defaultOpen,
  keywords: route.keywords,
}));

export const SUITE_NAV_GROUPS: SuiteNavGroup[] = GROUP_DEFS.map((group) => ({
  ...group,
  items: SUITE_NAV_ITEMS.filter((item) => item.groupId === group.id),
}));

export const DEFAULT_EXPANDED_GROUPS: ShellGroupId[] = SUITE_NAV_GROUPS.filter((group) => group.defaultOpen).map((group) => group.id);
export const DEFAULT_WORKSPACE_PATHS = SUITE_ROUTE_REGISTRY.filter((route) => route.defaultOpen).map((route) => route.to);
export const DEFAULT_PINNED_PATHS = SUITE_ROUTE_REGISTRY.filter((route) => route.defaultPinned).map((route) => route.to);

export function resolveRouteDefinition(pathname: string): SuiteRouteDefinition | undefined {
  return SUITE_ROUTE_REGISTRY.find((entry) => matchPath({ path: entry.path, end: entry.end }, pathname));
}

export function isKnownWorkspacePath(pathname: string): boolean {
  return Boolean(resolveRouteDefinition(pathname));
}

export function resolveNavSelection(pathname: string): string | null {
  return resolveRouteDefinition(pathname)?.navId ?? null;
}

export function canOpenAsDocument(pathname: string): boolean {
  return resolveRouteDefinition(pathname)?.workspaceKind === "document";
}

export function resolveWorkspaceMeta(pathname: string, override?: WorkspaceMetaOverride): ResolvedWorkspaceMeta {
  const route = resolveRouteDefinition(pathname);
  const title = override?.title ?? route?.meta.title ?? DEFAULT_HEADER_META.title;
  const subtitle = override?.subtitle ?? route?.meta.subtitle ?? DEFAULT_HEADER_META.subtitle;
  const crumbs = override?.crumbs ?? route?.meta.crumbs ?? DEFAULT_HEADER_META.crumbs;
  const label = override?.label ?? route?.label ?? title;
  const tabLabel = override?.tabLabel ?? label;
  const description = override?.description ?? route?.description ?? subtitle;
  const iconKey = override?.iconKey ?? route?.iconKey ?? "workspace";
  const keywords = `${route?.keywords ?? ""} ${override?.keywords ?? ""}`.trim();

  return {
    id: route?.id ?? pathname,
    path: pathname,
    navId: route?.navId ?? null,
    label,
    tabLabel,
    description,
    iconKey,
    groupId: route?.groupId ?? "mission-control",
    workspaceKind: route?.workspaceKind ?? "workspace",
    closable: override?.closable ?? route?.closable ?? true,
    defaultOpen: route?.defaultOpen ?? false,
    defaultPinned: route?.defaultPinned ?? false,
    keywords,
    title,
    subtitle,
    crumbs,
    mode: override?.mode ?? route?.meta.mode ?? DEFAULT_HEADER_META.mode,
  };
}

export function resolveHeaderMeta(pathname: string, override?: WorkspaceMetaOverride): HeaderMeta {
  const workspace = resolveWorkspaceMeta(pathname, override);
  return {
    title: workspace.title,
    subtitle: workspace.subtitle,
    crumbs: workspace.crumbs,
    mode: workspace.mode,
  };
}
