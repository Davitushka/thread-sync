import { useEffect, useMemo, useRef, useState } from "react";
import { matchPath, useLocation, useNavigate } from "react-router-dom";
import { listCases, uiConfig, type Case, type UiConfig } from "../api";
import { shortDateTime } from "../dashboard-utils";
import { DASHBOARDS, grafanaDashboardUrl } from "../dashboard-catalog";
import { resolveHeaderMeta, SUITE_NAV_ITEMS } from "../suite-meta";

type CommandAction =
  | {
      id: string;
      title: string;
      subtitle: string;
      section: string;
      keywords: string;
      run: () => void;
    }
  | {
      id: string;
      title: string;
      subtitle: string;
      section: string;
      keywords: string;
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

export default function CommandPalette({ actor }: Props) {
  const navigate = useNavigate();
  const location = useLocation();
  const [open, setOpen] = useState(false);
  const [query, setQuery] = useState("");
  const [selected, setSelected] = useState(0);
  const [config, setConfig] = useState<UiConfig | null>(null);
  const [recentCases, setRecentCases] = useState<Case[]>([]);
  const [casesLoaded, setCasesLoaded] = useState(false);
  const [casesError, setCasesError] = useState<string | null>(null);
  const inputRef = useRef<HTMLInputElement | null>(null);
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
      if ((event.ctrlKey || event.metaKey) && event.key.toLowerCase() === "k") {
        event.preventDefault();
        setOpen((value) => !value);
        return;
      }
      if (!openRef.current) {
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

  const currentMeta = useMemo(() => resolveHeaderMeta(location.pathname), [location.pathname]);
  const context = useMemo(() => routeContext(location.pathname, location.search), [location.pathname, location.search]);

  const actions = useMemo<CommandAction[]>(() => {
    const items: CommandAction[] = [];

    items.push(
      ...SUITE_NAV_ITEMS.map((item) => ({
        id: `nav:${item.to}`,
        title: `Open ${item.label}`,
        subtitle: item.description,
        section: "Navigate",
        keywords: `${item.label} route page ${item.to}`,
        run: () => {
          navigate(item.to);
          setOpen(false);
        },
      }))
    );

    items.push({
      id: "nav:current",
      title: `You are in ${currentMeta.title}`,
      subtitle: currentMeta.subtitle,
      section: "Context",
      keywords: `${location.pathname} current route`,
      run: () => setOpen(false),
    });

    if (context.overview || context.infrastructure || context.operations || context.dataQuality) {
      items.push({
        id: `context:refresh:${location.pathname}`,
        title: "Refresh current workspace",
        subtitle: `Reload ${currentMeta.title.toLowerCase()} and keep the current route context.`,
        section: "On this page",
        keywords: `refresh reload ${location.pathname}`,
        run: () => {
          window.location.reload();
        },
      });
    }

    if (context.overview) {
      items.push({
        id: "context:overview-ops",
        title: "Open operations center",
        subtitle: "Pivot from the SOC overview to the native pipeline operations workspace.",
        section: "On this page",
        keywords: "overview operations center pipeline",
        run: () => {
          navigate("/operations");
          setOpen(false);
        },
      });
      items.push({
        id: "context:overview-quality",
        title: "Open data quality",
        subtitle: "Jump from the overview into the trust layer for completeness and ingest lag.",
        section: "On this page",
        keywords: "overview data quality trust",
        run: () => {
          navigate("/data-quality");
          setOpen(false);
        },
      });
    }

    if (context.infrastructure) {
      items.push({
        id: "context:infra-ops",
        title: "Switch to operations center",
        subtitle: "Move from host-level health to pipeline and service flow monitoring.",
        section: "On this page",
        keywords: "infrastructure operations pipeline",
        run: () => {
          navigate("/operations");
          setOpen(false);
        },
      });
    }

    if (context.operations) {
      items.push({
        id: "context:ops-quality",
        title: "Open data quality view",
        subtitle: "Pivot from throughput and uptime into trust/completeness signals.",
        section: "On this page",
        keywords: "operations data quality trust",
        run: () => {
          navigate("/data-quality");
          setOpen(false);
        },
      });
      items.push({
        id: "context:ops-prom",
        title: "Open Prometheus for operations deep-dive",
        subtitle: "Use raw PromQL when the native operations charts are not enough.",
        section: "On this page",
        keywords: "operations prometheus promql",
        href: config?.links.prometheus || "#",
        external: true,
      });
    }

    if (context.dataQuality) {
      items.push({
        id: "context:quality-events",
        title: "Inspect raw events behind quality issues",
        subtitle: "Jump into native event search to validate source IP completeness and lag suspicions.",
        section: "On this page",
        keywords: "data quality events inspect",
        run: () => {
          navigate("/events");
          setOpen(false);
        },
      });
      items.push({
        id: "context:quality-ops",
        title: "Return to operations center",
        subtitle: "Move back to service uptime and throughput after trust analysis.",
        section: "On this page",
        keywords: "data quality operations",
        run: () => {
          navigate("/operations");
          setOpen(false);
        },
      });
    }

    if (context.events) {
      items.push({
        id: "context:events-clear",
        title: "Clear event search filters",
        subtitle: "Reset the current event route back to the base search screen.",
        section: "On this page",
        keywords: "events clear filters search",
        run: () => {
          navigate("/events");
          setOpen(false);
        },
      });
      if (context.eventQuery) {
        items.push({
          id: "context:events-repeat",
          title: `Repeat current event search for "${context.eventQuery}"`,
          subtitle: "Keep the current route query but jump directly back into the search view.",
          section: "On this page",
          keywords: `events query ${context.eventQuery}`,
          run: () => {
            navigate(`/events?q=${encodeURIComponent(context.eventQuery)}`);
            setOpen(false);
          },
        });
      }
    }

    if (context.alerts) {
      items.push({
        id: "context:alerts-events",
        title: "Pivot from alerts to event search",
        subtitle: "Move from triage inbox to raw event hunt for validation and context.",
        section: "On this page",
        keywords: "alerts events pivot",
        run: () => {
          navigate("/events");
          setOpen(false);
        },
      });
    }

    if (context.detections) {
      items.push({
        id: "context:detections-alerts",
        title: "Open alert inbox from detections",
        subtitle: "Move from firing rules to the triage queue.",
        section: "On this page",
        keywords: "detections alerts triage",
        run: () => {
          navigate("/alerts");
          setOpen(false);
        },
      });
    }

    if (context.cases) {
      items.push({
        id: "context:cases-new",
        title: "Open case queue and create a new case",
        subtitle: "Use the queue workspace with the case modal and actor context.",
        section: "On this page",
        keywords: "cases create new",
        run: () => {
          setOpen(false);
        },
      });
    }

    if (context.caseDetail?.params.id) {
      items.push({
        id: `context:case-investigate:${context.caseDetail.params.id}`,
        title: "Open investigation workbench",
        subtitle: "Continue from case detail into the investigation workspace.",
        section: "On this page",
        keywords: `case ${context.caseDetail.params.id} investigate`,
        run: () => {
          navigate(`/cases/${context.caseDetail?.params.id}/investigate`);
          setOpen(false);
        },
      });
    }

    if (context.investigation?.params.id) {
      items.push({
        id: `context:investigation-back:${context.investigation.params.id}`,
        title: "Back to case detail",
        subtitle: "Return from investigation to structured case management.",
        section: "On this page",
        keywords: `investigation case detail ${context.investigation.params.id}`,
        run: () => {
          navigate(`/cases/${context.investigation?.params.id}`);
          setOpen(false);
        },
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
          run: () => {
            navigate(entry.path);
            setOpen(false);
          },
        });
      } else if (entry.kind === "grafana" && entry.uid && config?.links.grafana) {
        const href = grafanaDashboardUrl(config.links.grafana, entry.uid, "now-24h", false);
        items.push({
          id: `dashboard:grafana:${entry.id}`,
          title: `Open ${entry.title} in Grafana`,
          subtitle: `${entry.badge}. ${entry.description}`,
          section: "Dashboards",
          keywords: `${entry.group} grafana ${entry.uid}`,
          href,
          external: true,
        });
      }
    }

    if (config?.links.grafana) {
      items.push({
        id: "tool:grafana",
        title: "Open Grafana root",
        subtitle: "Jump into the full dashboard catalog and advanced exploration.",
        section: "External tools",
        keywords: "grafana dashboards root",
        href: config.links.grafana,
        external: true,
      });
    }
    if (config?.links.prometheus) {
      items.push({
        id: "tool:prometheus",
        title: "Open Prometheus",
        subtitle: "Inspect raw metrics and PromQL output directly.",
        section: "External tools",
        keywords: "prometheus metrics query",
        href: config.links.prometheus,
        external: true,
      });
    }
    if (config?.links.alertmanager) {
      items.push({
        id: "tool:alertmanager",
        title: "Open Alertmanager",
        subtitle: "Check silences, routes and raw alert payloads.",
        section: "External tools",
        keywords: "alertmanager silences routing",
        href: config.links.alertmanager,
        external: true,
      });
    }
    if (config?.links.case_management) {
      items.push({
        id: "tool:case-mgmt",
        title: "Open Case Management service",
        subtitle: "Inspect the standalone case-management backend.",
        section: "External tools",
        keywords: "case management service backend",
        href: config.links.case_management,
        external: true,
      });
    }

    items.push({
      id: "action:new-case",
      title: "Go to case queue and create a new case",
      subtitle: `Current actor: ${actor || "analyst"}. Use the case modal from the queue view.`,
      section: "Quick actions",
      keywords: `new case create ${actor}`,
      run: () => {
        navigate("/cases");
        setOpen(false);
      },
    });

    items.push({
      id: "action:search-events",
      title: "Open event search",
      subtitle: queryValue ? `Search later for "${queryValue}" with native pivots.` : "Start from native event search and investigation pivots.",
      section: "Quick actions",
      keywords: "event search hunt clickhouse",
      run: () => {
        if (queryValue) {
          navigate(`/events?q=${encodeURIComponent(queryValue)}`);
        } else {
          navigate("/events");
        }
        setOpen(false);
      },
    });

    for (const item of recentCases) {
      items.push({
        id: `case:${item.id}`,
        title: `Open ${item.display_key}`,
        subtitle: `${item.title} · ${item.status} · ${item.severity} · updated ${shortDateTime(item.updated_at)}`,
        section: "Recent cases",
        keywords: `${item.display_key} ${item.title} ${item.severity} ${item.status} ${item.assignee ?? ""}`,
        run: () => {
          navigate(`/cases/${item.id}`);
          setOpen(false);
        },
      });
      items.push({
        id: `case-investigate:${item.id}`,
        title: `Investigate ${item.display_key}`,
        subtitle: `Open the investigation workbench for ${item.title}.`,
        section: "Recent cases",
        keywords: `${item.display_key} investigation workbench`,
        run: () => {
          navigate(`/cases/${item.id}/investigate`);
          setOpen(false);
        },
      });
    }

    return items;
  }, [
    actor,
    config,
    context.alerts,
    context.caseDetail,
    context.cases,
    context.dataQuality,
    context.dashboards,
    context.detections,
    context.eventQuery,
    context.events,
    context.infrastructure,
    context.investigation,
    context.operations,
    context.overview,
    currentMeta.subtitle,
    currentMeta.title,
    location.pathname,
    navigate,
    queryValue,
    recentCases,
  ]);

  const filtered = useMemo(() => {
    return actions
      .map((action) => ({ action, score: scoreCommand(queryValue, action) }))
      .filter((item) => item.score > 0)
      .sort((a, b) => b.score - a.score || a.action.title.localeCompare(b.action.title))
      .map((item) => item.action)
      .slice(0, 18);
  }, [actions, queryValue]);

  useEffect(() => {
    setSelected(0);
  }, [query]);

  useEffect(() => {
    if (!open) return;

    const onKeyDown = (event: KeyboardEvent) => {
      if (!open) return;
      if (event.key === "ArrowDown") {
        event.preventDefault();
        setSelected((value) => (filtered.length ? (value + 1) % filtered.length : 0));
      } else if (event.key === "ArrowUp") {
        event.preventDefault();
        setSelected((value) => (filtered.length ? (value - 1 + filtered.length) % filtered.length : 0));
      } else if (event.key === "Enter") {
        const current = filtered[selected];
        if (!current) return;
        event.preventDefault();
        if ("href" in current) {
          openExternal(current.href);
          setOpen(false);
        } else {
          current.run();
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
            <p>Keyboard-first navigation, pivots and external tool access across the suite.</p>
          </div>
          <span className="command-kbd">Esc</span>
        </div>

        <label className="command-search">
          <span>Search commands</span>
          <input
            ref={inputRef}
            value={query}
            onChange={(event) => setQuery(event.target.value)}
            placeholder="overview, alerts, grafana, case 42, investigate..."
          />
        </label>

        <div className="command-hints">
          <span className="token">Ctrl+K open</span>
          <span className="token">Enter run</span>
          <span className="token">Arrows move</span>
          {casesError ? <span className="token">Recent cases unavailable</span> : null}
        </div>

        <div className="command-results" role="listbox" aria-label="Command results">
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
              <p>Try route names, tools, dashboards, or a case identifier.</p>
            </div>
          )}
        </div>
      </section>
    </div>
  );
}
