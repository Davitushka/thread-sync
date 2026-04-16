import { useEffect, useMemo, useRef, useState } from "react";
import { matchPath, useLocation, useNavigate } from "react-router-dom";
import { listCases, uiConfig, type Case, type UiConfig } from "../api";
import { shortDateTime } from "../dashboard-utils";
import { DASHBOARDS, grafanaDashboardUrl } from "../dashboard-catalog";
import { resolveWorkspaceMeta, SUITE_NAV_GROUPS, SUITE_NAV_ITEMS } from "../suite-meta";
import { useChartMotion } from "./ChartMotionContext";
import { usePerfDebug } from "./PerfDebugContext";
import { useSuiteCommandContext } from "./SuiteCommandContext";
import { useWorkspaceShell } from "./WorkspaceShellContext";

type CommandAction =
  | {
      id: string;
      title: string;
      subtitle: string;
      section: string;
      keywords: string;
      priority?: number;
      run: () => void;
    }
  | {
      id: string;
      title: string;
      subtitle: string;
      section: string;
      keywords: string;
      priority?: number;
      href: string;
      external?: boolean;
    };

function normalize(value: string) {
  return value.toLowerCase().trim();
}

function scoreCommand(query: string, action: CommandAction) {
  if (!query) return 1;
  const haystack = `${action.title} ${action.subtitle} ${action.section} ${action.keywords}`.toLowerCase();
  if (haystack.startsWith(query)) return 100;
  if (action.title.toLowerCase().startsWith(query)) return 90;
  if (haystack.includes(query)) return 70;
  return 0;
}

function openExternal(url: string) {
  window.open(url, "_blank", "noopener,noreferrer");
}

function trimSlashPath(value: string) {
  return value.replace(/\/+$/, "");
}

function vectorIngestCurl(base: string) {
  const root = trimSlashPath(base || "http://localhost:8080");
  return `curl -sS -X POST "${root}/logs" -H "Content-Type: application/x-ndjson" --data-binary '{"event_type":"palette-demo","severity":"info","message":"smoke"}\\n'`;
}

const COMPOSE_LOGGEN_RESTART = "docker compose -f deploy/docker/docker-compose.yml restart log-generator";
const COMPOSE_STRESS_RESTART = "docker compose -f deploy/docker/docker-compose.yml restart siem-stress";

async function copyPlainText(label: string, text: string) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    window.prompt(`Copy (${label})`, text);
  }
}

/** Physical key (US QWERTY position) — works when OS layout is Russian, German, etc. */
function isCommandPaletteHotkey(event: KeyboardEvent) {
  return (event.ctrlKey || event.metaKey) && !event.altKey && event.code === "KeyK";
}

function isOperatorDeckHotkey(event: KeyboardEvent) {
  return event.altKey && !event.ctrlKey && !event.metaKey && event.code === "KeyO";
}

function isActivateChoiceKey(event: KeyboardEvent) {
  return event.code === "Enter" || event.code === "NumpadEnter";
}

function routeContext(pathname: string, search: string) {
  const params = new URLSearchParams(search);
  return {
    overview: pathname === "/",
    infrastructure: pathname === "/infrastructure",
    operations: pathname === "/operations",
    dataQuality: pathname === "/data-quality",
    dashboards: pathname === "/dashboards",
    alerts: pathname === "/alerts",
    detections: pathname === "/detections",
    events: pathname === "/events",
    cases: pathname === "/cases",
    caseDetail: matchPath("/cases/:id", pathname),
    investigation: matchPath("/cases/:id/investigate", pathname),
    eventQuery: params.get("q")?.trim() ?? "",
    eventSeverity: params.get("severity")?.trim() ?? "",
    eventSource: params.get("source_type")?.trim() ?? "",
  };
}

type Props = {
  actor: string;
};

const GROUP_LABELS = new Map(SUITE_NAV_GROUPS.map((group) => [group.id, group.label]));

export default function CommandPalette({ actor }: Props) {
  const navigate = useNavigate();
  const location = useLocation();
  const { chartAnimationsEnabled, setChartAnimationsEnabled } = useChartMotion();
  const { perfOverlayEnabled, setPerfOverlayEnabled } = usePerfDebug();
  const { pageCommands } = useSuiteCommandContext();
  const {
    tabs,
    activePath,
    activeWorkspace,
    recentPaths,
    openOrFocusWorkspace,
    closeWorkspace,
    pinWorkspace,
    unpinWorkspace,
    reopenRecentWorkspace,
  } = useWorkspaceShell();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const [config, setConfig] = useState<UiConfig | null>(null);
  const [recentCases, setRecentCases] = useState<Case[]>([]);
  const [casesLoaded, setCasesLoaded] = useState(false);
  const [casesError, setCasesError] = useState<string | null>(null);
  const [operatorDeck, setOperatorDeck] = useState(false);
  const inputRef = useRef<HTMLInputElement | null>(null);
  const resultsRef = useRef<HTMLDivElement | null>(null);
  const openRef = useRef(open);

  useEffect(() => {
    openRef.current = open;
  }, [open]);

  useEffect(() => {
    uiConfig()
      .then(setConfig)
      .catch(() => undefined);
  }, []);

  useEffect(() => {
    const openHandler = () => setOpen(true);
    window.addEventListener("suite:open-command-palette", openHandler);
    return () => window.removeEventListener("suite:open-command-palette", openHandler);
  }, []);

  useEffect(() => {
    const onKeyDown = (event: KeyboardEvent) => {
      const target = event.target as HTMLElement | null;
      const typingContext =
        target instanceof HTMLInputElement ||
        target instanceof HTMLTextAreaElement ||
        target?.isContentEditable === true ||
        target instanceof HTMLSelectElement;
      if (isCommandPaletteHotkey(event)) {
        event.preventDefault();
        setOpen((value) => !value);
        return;
      }
      if (!openRef.current) return;
      if (isOperatorDeckHotkey(event)) {
        event.preventDefault();
        setOperatorDeck((value) => !value);
        return;
      }
      if (event.key === "Escape") {
        event.preventDefault();
        setOpen(false);
        return;
      }
      if (typingContext && target !== inputRef.current) {
        return;
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, []);

  useEffect(() => {
    if (!open) {
      setQuery("");
      setSelected(0);
      setOperatorDeck(false);
      return;
    }
    inputRef.current?.focus();
    inputRef.current?.select();
  }, [open]);

  useEffect(() => {
    if (!open || casesLoaded) return;
    listCases({ limit: "8" })
      .then((result) => {
        setRecentCases(result.cases);
        setCasesLoaded(true);
      })
      .catch((error) => {
        setCasesError(String(error));
        setCasesLoaded(true);
      });
  }, [casesLoaded, open]);

  const queryValue = normalize(query);
  const context = useMemo(() => routeContext(location.pathname, location.search), [location.pathname, location.search]);
  const activeTab = tabs.find((tab) => tab.path === activePath) ?? null;

  const openWorkspaceUrl = (path: string, fullUrl?: string) => {
    openOrFocusWorkspace(path);
    if (fullUrl && fullUrl !== path) {
      navigate(fullUrl);
    }
  };

  const actions = useMemo<CommandAction[]>(() => {
    const items: CommandAction[] = [];

    items.push({
      id: "workspace:current",
      title: `Workspace: ${activeWorkspace.title}`,
      subtitle: activeWorkspace.subtitle,
      section: "Current workspace",
      keywords: `${location.pathname} ${activeWorkspace.keywords}`,
      priority: 5,
      run: () => setOpen(false),
    });

    items.push(
      ...SUITE_NAV_ITEMS.map((item) => ({
        id: `nav:${item.to}`,
        title: `Open ${item.label}`,
        subtitle: item.description,
        section: GROUP_LABELS.get(item.groupId) ?? "Explorer",
        keywords: `${item.keywords} ${item.to}`,
        priority: item.defaultPinned ? 72 : 60,
        run: () => openOrFocusWorkspace(item.to),
      }))
    );

    items.push(
      ...tabs.map((tab) => ({
        id: `tab:${tab.path}`,
        title: `${tab.path === activePath ? "Focus" : "Switch to"} ${tab.tabLabel}`,
        subtitle: `${tab.pinned ? "Pinned" : "Open"} tab in ${GROUP_LABELS.get(tab.groupId) ?? "workspace"}.`,
        section: "Open tabs",
        keywords: `${tab.tabLabel} ${tab.path} open tab ${tab.groupId}`,
        priority: tab.path === activePath ? 96 : 90,
        run: () => openOrFocusWorkspace(tab.path),
      }))
    );

    if (activeTab) {
      items.push({
        id: `tab:pin:${activePath}`,
        title: activeTab.pinned ? "Unpin active tab" : "Pin active tab",
        subtitle: activeTab.pinned
          ? "Remove the active workspace from the pinned tab set."
          : "Keep the active workspace pinned in the desktop shell.",
        section: "Open tabs",
        keywords: `${activeTab.tabLabel} pin unpin tab`,
        priority: 97,
        run: () => (activeTab.pinned ? unpinWorkspace(activePath) : pinWorkspace(activePath)),
      });

      if (activeTab.closable && !activeTab.pinned) {
        items.push({
          id: `tab:close:${activePath}`,
          title: "Close active tab",
          subtitle: "Close the active workspace and focus the nearest remaining tab.",
          section: "Open tabs",
          keywords: `${activeTab.tabLabel} close tab`,
          priority: 94,
          run: () => closeWorkspace(activePath),
        });
      }
    }

    items.push(
      ...recentPaths
        .filter((path) => path !== activePath)
        .slice(0, 6)
        .map((path) => {
          const meta = resolveWorkspaceMeta(path);
          return {
            id: `recent:${path}`,
            title: `Reopen ${meta.tabLabel}`,
            subtitle: `Restore the recent workspace for ${meta.title.toLowerCase()}.`,
            section: "Recent workspaces",
            keywords: `${meta.keywords} ${path} recent reopen`,
            priority: 74,
            run: () => reopenRecentWorkspace(path),
          };
        })
    );

    items.push(
      ...pageCommands.flatMap((command): CommandAction[] => {
        const base = {
          id: command.id,
          title: command.title,
          subtitle: command.subtitle,
          section: command.section || "Current page",
          keywords: command.keywords || "",
          priority: command.priority ?? 90,
        };
        if (command.href) {
          return [{ ...base, href: command.href, external: command.external }];
        }
        if (command.run) {
          const runFn = command.run;
          return [{ ...base, run: () => void runFn() }];
        }
        return [];
      })
    );

    items.push({
      id: `context:refresh:${location.pathname}`,
      title: "Refresh current workspace",
      subtitle: `Reload ${activeWorkspace.title.toLowerCase()} and keep the current route context.`,
      section: "Current workspace",
      keywords: `refresh reload ${location.pathname}`,
      priority: 68,
      run: () => window.location.reload(),
    });

    items.push({
      id: "suite:open-settings",
      title: "Open settings",
      subtitle: "Open global suite settings (actor, refresh, chart animations).",
      section: "Display",
      keywords: "settings preferences refresh actor charts animation",
      priority: 66,
      run: () => {
        window.dispatchEvent(new CustomEvent("suite:open-settings"));
        setOpen(false);
      },
    });

    items.push({
      id: "suite:chart-animations-toggle",
      title: chartAnimationsEnabled ? "Turn off chart animations" : "Turn on chart animations",
      subtitle: chartAnimationsEnabled
        ? "Smoother scrolling in SIEM Operator WebView; ECharts updates without motion."
        : "Full ECharts line and gauge animations (heavier while auto-refresh runs).",
      section: "Display",
      keywords: "charts animation echarts motion performance operator webview smooth sidebar",
      priority: 64,
      run: () => {
        setChartAnimationsEnabled(!chartAnimationsEnabled);
        setOpen(false);
      },
    });
    items.push({
      id: "suite:perf-overlay-toggle",
      title: perfOverlayEnabled ? "Hide performance overlay" : "Show performance overlay",
      subtitle: "FPS and long-frame counters for stutter diagnostics.",
      section: "Display",
      keywords: "perf fps frame latency overlay debug",
      priority: 63,
      run: () => {
        setPerfOverlayEnabled(!perfOverlayEnabled);
        setOpen(false);
      },
    });

    if (context.overview) {
      items.push({
        id: "context:overview-ops",
        title: "Open operations center",
        subtitle: "Pivot from the SOC overview to the native pipeline operations workspace.",
        section: "Current workspace",
        keywords: "overview operations center pipeline",
        priority: 82,
        run: () => openOrFocusWorkspace("/operations"),
      });
      items.push({
        id: "context:overview-quality",
        title: "Open data quality",
        subtitle: "Jump from the overview into the trust layer for completeness and ingest lag.",
        section: "Current workspace",
        keywords: "overview data quality trust",
        priority: 81,
        run: () => openOrFocusWorkspace("/data-quality"),
      });
    }

    if (context.infrastructure) {
      items.push({
        id: "context:infra-ops",
        title: "Switch to operations center",
        subtitle: "Move from host-level health to pipeline and service flow monitoring.",
        section: "Current workspace",
        keywords: "infrastructure operations pipeline",
        priority: 80,
        run: () => openOrFocusWorkspace("/operations"),
      });
    }

    if (context.operations) {
      items.push({
        id: "context:ops-quality",
        title: "Open data quality view",
        subtitle: "Pivot from throughput and uptime into trust/completeness signals.",
        section: "Current workspace",
        keywords: "operations data quality trust",
        priority: 80,
        run: () => openOrFocusWorkspace("/data-quality"),
      });
      items.push({
        id: "context:ops-prom",
        title: "Open Prometheus for operations deep-dive",
        subtitle: "Use raw PromQL when the native operations charts are not enough.",
        section: "Current workspace",
        keywords: "operations prometheus promql",
        priority: 70,
        href: config?.links.prometheus || "#",
        external: true,
      });
    }

    if (context.dataQuality) {
      items.push({
        id: "context:quality-events",
        title: "Inspect raw events behind quality issues",
        subtitle: "Jump into native event search to validate source IP completeness and lag suspicions.",
        section: "Current workspace",
        keywords: "data quality events inspect",
        priority: 80,
        run: () => openOrFocusWorkspace("/events"),
      });
      items.push({
        id: "context:quality-ops",
        title: "Return to operations center",
        subtitle: "Move back to service uptime and throughput after trust analysis.",
        section: "Current workspace",
        keywords: "data quality operations",
        priority: 79,
        run: () => openOrFocusWorkspace("/operations"),
      });
    }

    if (context.events) {
      items.push({
        id: "context:events-clear",
        title: "Clear event search filters",
        subtitle: "Reset the current event route back to the base search screen.",
        section: "Current workspace",
        keywords: "events clear filters search",
        priority: 76,
        run: () => openWorkspaceUrl("/events"),
      });
      if (context.eventQuery) {
        items.push({
          id: "context:events-repeat",
          title: `Repeat current event search for "${context.eventQuery}"`,
          subtitle: "Keep the current route query but jump directly back into the search view.",
          section: "Current workspace",
          keywords: `events query ${context.eventQuery}`,
          priority: 75,
          run: () => openWorkspaceUrl("/events", `/events?q=${encodeURIComponent(context.eventQuery)}`),
        });
      }
    }

    if (context.alerts) {
      items.push({
        id: "context:alerts-events",
        title: "Pivot from alerts to event search",
        subtitle: "Move from triage inbox to raw event hunt for validation and context.",
        section: "Current workspace",
        keywords: "alerts events pivot",
        priority: 76,
        run: () => openOrFocusWorkspace("/events"),
      });
    }

    if (context.detections) {
      items.push({
        id: "context:detections-alerts",
        title: "Open alert inbox from detections",
        subtitle: "Move from firing rules to the triage queue.",
        section: "Current workspace",
        keywords: "detections alerts triage",
        priority: 76,
        run: () => openOrFocusWorkspace("/alerts"),
      });
    }

    if (context.caseDetail?.params.id) {
      items.push({
        id: `context:case-investigate:${context.caseDetail.params.id}`,
        title: "Open investigation workbench",
        subtitle: "Continue from case detail into the investigation workspace.",
        section: "Current workspace",
        keywords: `case ${context.caseDetail.params.id} investigate`,
        priority: 84,
        run: () => openOrFocusWorkspace(`/cases/${context.caseDetail?.params.id}/investigate`),
      });
    }

    if (context.investigation?.params.id) {
      items.push({
        id: `context:investigation-back:${context.investigation.params.id}`,
        title: "Back to case detail",
        subtitle: "Return from investigation to structured case management.",
        section: "Current workspace",
        keywords: `investigation case detail ${context.investigation.params.id}`,
        priority: 84,
        run: () => openOrFocusWorkspace(`/cases/${context.investigation?.params.id}`),
      });
    }

    for (const entry of DASHBOARDS) {
      if (entry.kind === "native" && entry.path) {
        items.push({
          id: `dashboard:native:${entry.id}`,
          title: `Open ${entry.title}`,
          subtitle: `${entry.badge}. ${entry.description}`,
          section: "Dashboards",
          keywords: `${entry.group} ${entry.status} ${entry.badge}`,
          priority: 62,
          run: () => openOrFocusWorkspace(entry.path || "/dashboards"),
        });
      } else if (entry.kind === "grafana" && entry.uid && config?.links.grafana) {
        const href = grafanaDashboardUrl(config.links.grafana, entry.uid, "now-24h", false);
        items.push({
          id: `dashboard:grafana:${entry.id}`,
          title: `Open ${entry.title} in Grafana`,
          subtitle: `${entry.badge}. ${entry.description}`,
          section: "Dashboards",
          keywords: `${entry.group} grafana ${entry.uid}`,
          priority: 58,
          href,
          external: true,
        });
      }
    }

    items.push({
      id: "action:new-case",
      title: "Go to case queue and create a new case",
      subtitle: `Current actor: ${actor || "analyst"}. Open the response workspace and use the case modal.`,
      section: "Quick actions",
      keywords: `new case create ${actor}`,
      priority: 73,
      run: () => openOrFocusWorkspace("/cases"),
    });

    items.push({
      id: "action:search-events",
      title: "Open event search",
      subtitle: queryValue ? `Search later for "${queryValue}" with native pivots.` : "Start from native event search and investigation pivots.",
      section: "Quick actions",
      keywords: "event search hunt clickhouse",
      priority: 72,
      run: () => {
        if (queryValue) {
          openWorkspaceUrl("/events", `/events?q=${encodeURIComponent(queryValue)}`);
        } else {
          openOrFocusWorkspace("/events");
        }
      },
    });

    for (const item of recentCases) {
      items.push({
        id: `case:${item.id}`,
        title: `Open ${item.display_key}`,
        subtitle: `${item.title} · ${item.status} · ${item.severity} · updated ${shortDateTime(item.updated_at)}`,
        section: "Recent cases",
        keywords: `${item.display_key} ${item.title} ${item.severity} ${item.status} ${item.assignee ?? ""}`,
        priority: 78,
        run: () => openOrFocusWorkspace(`/cases/${item.id}`),
      });
      items.push({
        id: `case-investigate:${item.id}`,
        title: `Investigate ${item.display_key}`,
        subtitle: `Open the investigation workbench for ${item.title}.`,
        section: "Recent cases",
        keywords: `${item.display_key} investigation workbench`,
        priority: 77,
        run: () => openOrFocusWorkspace(`/cases/${item.id}/investigate`),
      });
    }

    return items;
  }, [
    activePath,
    activeTab,
    activeWorkspace,
    actor,
    config,
    context.alerts,
    context.caseDetail,
    context.dataQuality,
    context.detections,
    context.eventQuery,
    context.events,
    context.infrastructure,
    context.investigation,
    context.operations,
    context.overview,
    location.pathname,
    location.search,
    navigate,
    openOrFocusWorkspace,
    pageCommands,
    pinWorkspace,
    queryValue,
    recentCases,
    recentPaths,
    reopenRecentWorkspace,
    chartAnimationsEnabled,
    closeWorkspace,
    setChartAnimationsEnabled,
    setPerfOverlayEnabled,
    unpinWorkspace,
    tabs,
    perfOverlayEnabled,
  ]);

  const filtered = useMemo(() => {
    return actions
      .map((action) => ({ action, score: scoreCommand(queryValue, action) }))
      .filter((item) => item.score > 0)
      .sort(
        (a, b) =>
          b.score - a.score ||
          (b.action.priority ?? 0) - (a.action.priority ?? 0) ||
          a.action.title.localeCompare(b.action.title)
      )
      .map((item) => item.action)
      .slice(0, 56);
  }, [actions, queryValue]);

  useEffect(() => {
    if (!open) return;
    const root = resultsRef.current;
    if (!root) return;
    const active = root.querySelector(".command-item.active");
    if (active instanceof HTMLElement) {
      active.scrollIntoView({ block: "nearest", behavior: "smooth" });
    }
  }, [open, selected, filtered]);

  useEffect(() => {
    setSelected(0);
  }, [query]);

  useEffect(() => {
    if (!open) return;

    const onKeyDown = (event: KeyboardEvent) => {
      if (!open) return;
      if (event.code === "ArrowDown") {
        event.preventDefault();
        setSelected((value) => (filtered.length ? (value + 1) % filtered.length : 0));
      } else if (event.code === "ArrowUp") {
        event.preventDefault();
        setSelected((value) => (filtered.length ? (value - 1 + filtered.length) % filtered.length : 0));
      } else if (isActivateChoiceKey(event)) {
        const current = filtered[selected];
        if (!current) return;
        event.preventDefault();
        if ("href" in current) {
          openExternal(current.href);
          setOpen(false);
        } else {
          current.run();
          setOpen(false);
        }
      }
    };

    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [filtered, open, selected]);

  if (!open) return null;

  return (
    <div className="command-palette-backdrop" onClick={() => setOpen(false)}>
      <section className="command-palette" onClick={(event) => event.stopPropagation()}>
        <div className="command-palette-head">
          <div>
            <strong>Unified command palette</strong>
            <p>Keyboard-first navigation, workspace control and operational pivots across the suite.</p>
          </div>
          <span className="command-kbd">Esc</span>
        </div>

        <label className="command-search">
          <span>Search commands</span>
          <input
            ref={inputRef}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="overview, tabs, close tab, alerts, case SOC-001..."
          />
        </label>

        <div className="command-hints">
          <span className="token">Ctrl+K — same physical key as Latin K (any layout)</span>
          <span className="token">Enter run</span>
          <span className="token">↑↓ move</span>
          <span className="token">Alt+O — same physical key as Latin O</span>
          <span className="token">{tabs.length} tabs</span>
          {casesError ? <span className="token">Recent cases unavailable</span> : null}
        </div>

        <div className="command-operator-toggle">
          <button type="button" onClick={() => setOperatorDeck((value) => !value)}>
            {operatorDeck ? "Hide stack tools & hotkeys" : "Show stack tools & hotkeys"}
          </button>
          <span className="meta">Grafana, Prometheus, generators, Vector ingest — kept out of the main list on purpose.</span>
        </div>

        {operatorDeck ? (
          <div className="command-operator-panel">
            <div className="command-operator-section">
              <h4>Hotkeys</h4>
              <div className="command-hotkey-grid">
                <div>
                  <span>Ctrl+K / ⌘K</span> — open or close (physical <code>KeyK</code>, works in RU/DE/etc.)
                </div>
                <div>
                  <span>Alt+O</span> — toggle stack tools (physical <code>KeyO</code>)
                </div>
                <div>
                  <span>↑ ↓</span> — move selection (physical arrow keys; list scrolls)
                </div>
                <div>
                  <span>Enter</span> — run the highlighted command (main or numpad)
                </div>
                <div>
                  <span>Esc</span> — close the palette
                </div>
              </div>
            </div>

            <div className="command-operator-section">
              <h4>Observability & cases (opens in a new tab)</h4>
              <div className="command-operator-actions">
                {config?.links.grafana ? (
                  <a href={config.links.grafana} target="_blank" rel="noreferrer">
                    Grafana
                  </a>
                ) : null}
                {config?.links.siem_overview_dashboard ? (
                  <a href={config.links.siem_overview_dashboard} target="_blank" rel="noreferrer">
                    SIEM overview dashboard
                  </a>
                ) : null}
                {config?.links.prometheus ? (
                  <a href={config.links.prometheus} target="_blank" rel="noreferrer">
                    Prometheus
                  </a>
                ) : null}
                {config?.links.alertmanager ? (
                  <a href={config.links.alertmanager} target="_blank" rel="noreferrer">
                    Alertmanager
                  </a>
                ) : null}
                {config?.links.case_management ? (
                  <a href={config.links.case_management} target="_blank" rel="noreferrer">
                    Case management API
                  </a>
                ) : null}
              </div>
            </div>

            <div className="command-operator-section">
              <h4>Pipeline & load generators</h4>
              <p className="command-operator-note">
                Compose services <code>log-generator</code> and <code>siem-stress</code> ship NDJSON to Vector (
                <code>/logs</code>). Restart from the repo root to nudge traffic; tune env under{" "}
                <code>deploy/docker/docker-compose.yml</code>.
              </p>
              <div className="command-operator-actions">
                <button type="button" onClick={() => void copyPlainText("compose log-generator", COMPOSE_LOGGEN_RESTART)}>
                  Copy: restart log-generator
                </button>
                <button type="button" onClick={() => void copyPlainText("compose siem-stress", COMPOSE_STRESS_RESTART)}>
                  Copy: restart siem-stress
                </button>
                <button
                  type="button"
                  onClick={() =>
                    void copyPlainText(
                      "vector curl",
                      vectorIngestCurl(config?.links.vector_http_base ?? "http://localhost:8080")
                    )
                  }
                >
                  Copy: curl → Vector /logs
                </button>
                <button
                  type="button"
                  onClick={() =>
                    void copyPlainText(
                      "vector logs URL",
                      `${trimSlashPath(config?.links.vector_http_base ?? "http://localhost:8080")}/logs`
                    )
                  }
                >
                  Copy: ingest URL only
                </button>
                <a
                  href={trimSlashPath(config?.links.redpanda_admin || "http://localhost:9644")}
                  target="_blank"
                  rel="noreferrer"
                >
                  Redpanda admin HTTP
                </a>
              </div>
            </div>
          </div>
        ) : null}

        <div ref={resultsRef} className="command-results" role="listbox" aria-label="Command results">
          {filtered.length ? (
            filtered.map((item, index) => {
              const active = index === selected;
              return (
                <button
                  key={item.id}
                  type="button"
                  className={active ? "command-item active" : "command-item"}
                  onMouseEnter={() => setSelected(index)}
                  onClick={() => {
                    if ("href" in item) {
                      openExternal(item.href);
                      setOpen(false);
                    } else {
                      item.run();
                      setOpen(false);
                    }
                  }}
                >
                  <div>
                    <strong>{item.title}</strong>
                    <p>{item.subtitle}</p>
                  </div>
                  <div className="command-meta">
                    <span>{item.section}</span>
                    {"href" in item && item.external ? <span className="token">external</span> : null}
                  </div>
                </button>
              );
            })
          ) : (
            <div className="command-empty">
              <strong>No matching commands</strong>
              <p>Try workspace names, tabs, dashboards, cases — external links live under “stack tools”.</p>
            </div>
          )}
        </div>
      </section>
    </div>
  );
}
