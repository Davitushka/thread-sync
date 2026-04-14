import { useCallback, useEffect, useMemo, useState } from "react";
import { Link, useNavigate, useSearchParams } from "react-router-dom";
import {
  createCase,
  getEntityContext,
  getEvent,
  linkEvent,
  searchEvents,
  type EntityContext,
  type EventDetail,
  type EventRow,
  type EventSearchResponse,
} from "../api";
import AdaptivePaneLayout from "../components/AdaptivePaneLayout";
import DashboardToolbar from "../components/DashboardToolbar";
import { ObservabilityBarPanel, ObservabilityLinePanel } from "../components/echarts/ObservabilityCharts";
import { useActorState } from "../components/PageLayout";
import { usePublishPageCommands, type SuitePageCommand } from "../components/SuiteCommandContext";
import { formatCompact, shortDateTime } from "../dashboard-utils";

type Filters = {
  q: string;
  severity: string;
  source_type: string;
  host: string;
  source_ip: string;
  user_id: string;
  start: string;
  end: string;
};

const INITIAL_FILTERS: Filters = {
  q: "",
  severity: "",
  source_type: "",
  host: "",
  source_ip: "",
  user_id: "",
  start: "",
  end: "",
};

const TIME_PRESETS = [
  { id: "15m", label: "Last 15m", minutes: 15 },
  { id: "1h", label: "Last 1h", minutes: 60 },
  { id: "6h", label: "Last 6h", minutes: 360 },
  { id: "24h", label: "Last 24h", minutes: 1_440 },
];

function severityTone(value?: string) {
  return (value || "info").toLowerCase();
}

function priorityFromSeverity(value?: string) {
  const severity = severityTone(value);
  if (severity === "critical") return { label: "P1", tone: "critical" as const };
  if (severity === "error") return { label: "P2", tone: "high" as const };
  if (severity === "warning") return { label: "P3", tone: "medium" as const };
  return { label: "P4", tone: "low" as const };
}

function buildSearchString(next: Filters) {
  const params = new URLSearchParams();
  for (const [key, value] of Object.entries(next)) {
    if (value.trim()) {
      params.set(key, value.trim());
    }
  }
  return params.toString();
}

function applyRelativeWindow(minutes: number) {
  const end = new Date();
  const start = new Date(end.getTime() - minutes * 60_000);
  return {
    start: start.toISOString(),
    end: end.toISOString(),
  };
}

function formatFilterWindow(start?: string, end?: string) {
  if (!start || !end) return "Live query";
  const parsedStart = new Date(start);
  const parsedEnd = new Date(end);
  if (Number.isNaN(parsedStart.getTime()) || Number.isNaN(parsedEnd.getTime())) {
    return "Bounded window";
  }
  return `${parsedStart.toLocaleString([], {
    month: "short",
    day: "numeric",
    hour: "2-digit",
    minute: "2-digit",
  })} - ${parsedEnd.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })}`;
}

export default function EventsPage() {
  const navigate = useNavigate();
  const [searchParams] = useSearchParams();
  const [filters, setFilters] = useState<Filters>(INITIAL_FILTERS);
  const [results, setResults] = useState<EventSearchResponse | null>(null);
  const [selected, setSelected] = useState<EventDetail | null>(null);
  const [context, setContext] = useState<EntityContext | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const { actor, setActor } = useActorState();

  const activeQueryParams = useMemo(() => {
    const out: Record<string, string> = {};
    for (const key of Object.keys(INITIAL_FILTERS) as Array<keyof Filters>) {
      const value = searchParams.get(key);
      if (value?.trim()) out[key] = value.trim();
    }
    return out;
  }, [searchParams]);

  const fetchResults = useCallback(async (params: Record<string, string>) => {
    setLoading(true);
    setErr(null);
    try {
      const data = await searchEvents(params);
      setResults(data);
      setSelected(null);
      setContext(null);
    } catch (error) {
      setErr(String(error));
    } finally {
      setLoading(false);
    }
  }, []);

  const commitSearch = useCallback(
    (nextFilters: Filters) => {
      const nextParams = buildSearchString(nextFilters);
      navigate(nextParams ? `/events?${nextParams}` : "/events");
    },
    [navigate]
  );

  const applyFilters = useCallback(
    (patch: Partial<Filters>) => {
      const next = { ...filters, ...patch };
      setFilters(next);
      commitSearch(next);
    },
    [commitSearch, filters]
  );

  useEffect(() => {
    const next = { ...INITIAL_FILTERS };
    for (const key of Object.keys(next) as Array<keyof Filters>) {
      const value = searchParams.get(key);
      if (value) next[key] = value;
    }
    setFilters(next);
    if (!Object.keys(activeQueryParams).length) {
      // Default behavior: show the latest stream window instead of an empty workspace.
      void fetchResults({});
      return;
    }
    void fetchResults(activeQueryParams);
  }, [searchParams, activeQueryParams, fetchResults]);

  const clearWorkspace = useCallback(() => {
    setFilters(INITIAL_FILTERS);
    setResults(null);
    setSelected(null);
    setContext(null);
    setErr(null);
  }, []);

  const load = useCallback(
    async (e?: React.FormEvent) => {
      e?.preventDefault();
      commitSearch(filters);
    },
    [filters, commitSearch]
  );

  const openEvent = useCallback(async (row: EventRow) => {
    setErr(null);
    try {
      const detail = await getEvent(row.event_id);
      setSelected(detail);
      if (row.source_ip) {
        setContext(await getEntityContext("ip", row.source_ip));
      } else if (row.user_id) {
        setContext(await getEntityContext("user", row.user_id));
      } else if (row.host) {
        setContext(await getEntityContext("host", row.host));
      } else {
        setContext(null);
      }
    } catch (error) {
      setErr(String(error));
    }
  }, []);

  const selectedEntityValue = selected?.event.source_ip || selected?.event.user_id || selected?.event.host;
  const logRows = results?.rows ?? [];
  const hasActiveQuery = Object.keys(activeQueryParams).length > 0;
  const streamLabels = useMemo(
    () =>
      logRows.map((row) => {
        const parsed = new Date(row.timestamp);
        return Number.isNaN(parsed.getTime())
          ? row.timestamp
          : parsed.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit", second: "2-digit" });
      }),
    [logRows]
  );
  const topSourceTypes = useMemo(() => {
    const counts = new Map<string, number>();
    for (const row of logRows) {
      counts.set(row.source_type, (counts.get(row.source_type) ?? 0) + 1);
    }
    return Array.from(counts.entries())
      .sort((a, b) => b[1] - a[1])
      .slice(0, 4);
  }, [logRows]);
  const topEventTypes = useMemo(() => {
    const counts = new Map<string, number>();
    for (const row of logRows) {
      counts.set(row.event_type || "unknown", (counts.get(row.event_type || "unknown") ?? 0) + 1);
    }
    return Array.from(counts.entries())
      .sort((a, b) => b[1] - a[1])
      .slice(0, 5);
  }, [logRows]);
  const severityCounts = useMemo(() => {
    const output = { critical: 0, error: 0, warning: 0, info: 0 };
    for (const row of logRows) {
      const key = severityTone(row.severity) as keyof typeof output;
      if (key in output) {
        output[key] += 1;
      } else {
        output.info += 1;
      }
    }
    return output;
  }, [logRows]);
  const activeFilterChips = useMemo(
    () =>
      Object.entries(filters)
        .filter(([, value]) => value.trim())
        .map(([key, value]) => ({ key: key as keyof Filters, label: key.replace("_", " "), value })),
    [filters]
  );

  const promoteToCase = useCallback(
    async (row: EventRow) => {
      setErr(null);
      try {
        const created = await createCase(
          {
            title: `[event] ${row.source_type} ${row.host}`,
            description: row.message,
            severity: row.severity === "critical" ? "critical" : row.severity === "error" ? "high" : "medium",
          },
          actor
        );
        await linkEvent(created.id, row.event_id, `Promoted from event search (${row.timestamp})`, actor);
        navigate(`/cases/${created.id}`);
      } catch (error) {
        setErr(String(error));
      }
    },
    [actor, navigate]
  );

  const pageCommands = useMemo<SuitePageCommand[]>(() => {
    const commands: SuitePageCommand[] = [];

    if (Object.keys(activeQueryParams).length) {
      commands.push({
        id: "events:refresh",
        title: "Refresh current event search",
        subtitle: "Run the active native event query again and replace the current result set.",
        section: "Current event search",
        keywords: Object.values(activeQueryParams).join(" "),
        priority: 85,
        run: () => void fetchResults(activeQueryParams),
      });
    }

    if (Object.values(filters).some((value) => value.trim())) {
      commands.push({
        id: "events:clear",
        title: "Clear event filters",
        subtitle: "Reset all current event filters, selected row and entity context.",
        section: "Current event search",
        keywords: "events clear filters reset",
        priority: 90,
        run: () => {
          clearWorkspace();
          navigate("/events");
        },
      });
    }

    for (const preset of TIME_PRESETS) {
      commands.push({
        id: `events:window:${preset.id}`,
        title: `Apply ${preset.label} window`,
        subtitle: "Replace the current event search window with a relative time range preset.",
        section: "Current event search",
        keywords: `events time window ${preset.label}`,
        priority: 72,
        run: () => {
          applyFilters(applyRelativeWindow(preset.minutes));
        },
      });
    }

    if (selected) {
      commands.push(
        {
          id: `events:copy:${selected.event.event_id}`,
          title: "Copy selected event id",
          subtitle: "Copy the currently opened event identifier to the clipboard.",
          section: "Selected event",
          keywords: `${selected.event.event_id} copy event`,
          priority: 75,
          run: () => navigator.clipboard.writeText(selected.event.event_id),
        },
        {
          id: `events:promote:${selected.event.event_id}`,
          title: "Promote selected event to case",
          subtitle: "Create a case from the opened event and attach it to the case timeline.",
          section: "Selected event",
          keywords: `${selected.event.event_id} promote case`,
          priority: 100,
          run: () => promoteToCase(selected.event),
        }
      );

      if (selected.event.source_ip) {
        commands.push({
          id: `events:filter-ip:${selected.event.event_id}`,
          title: `Filter by IP ${selected.event.source_ip}`,
          subtitle: "Apply the selected event source IP as the active event search filter.",
          section: "Selected event",
          keywords: `${selected.event.source_ip} ip filter events`,
          priority: 92,
          run: () => applyFilters({ source_ip: selected.event.source_ip || "" }),
        });
      }
      if (selected.event.user_id) {
        commands.push({
          id: `events:filter-user:${selected.event.event_id}`,
          title: `Filter by user ${selected.event.user_id}`,
          subtitle: "Apply the selected event user identifier as the active event search filter.",
          section: "Selected event",
          keywords: `${selected.event.user_id} user filter events`,
          priority: 90,
          run: () => applyFilters({ user_id: selected.event.user_id || "" }),
        });
      }
      if (selected.event.host) {
        commands.push({
          id: `events:filter-host:${selected.event.event_id}`,
          title: `Filter by host ${selected.event.host}`,
          subtitle: "Apply the selected event host as the active event search filter.",
          section: "Selected event",
          keywords: `${selected.event.host} host filter events`,
          priority: 88,
          run: () => applyFilters({ host: selected.event.host || "" }),
        });
      }
    }

    if (context?.entity) {
      commands.push({
        id: `events:entity-copy:${context.entity.kind}:${context.entity.value}`,
        title: `Copy ${context.entity.kind} ${context.entity.value}`,
        subtitle: "Copy the current entity context value for reuse in hunts, tickets or chat.",
        section: "Entity context",
        keywords: `${context.entity.kind} ${context.entity.value} copy`,
        priority: 70,
        run: () => navigator.clipboard.writeText(context.entity.value),
      });
    }

    if (selectedEntityValue) {
      commands.push({
        id: `events:cases:${selected?.event.event_id ?? "selected"}`,
        title: `Search cases for ${selectedEntityValue}`,
        subtitle: "Pivot into case operations using the selected event entity as the search value.",
        section: "Selected event",
        keywords: `${selectedEntityValue} cases search`,
        priority: 84,
        run: () => navigate(`/cases?q=${encodeURIComponent(selectedEntityValue)}`),
      });
    }

    return commands;
  }, [activeQueryParams, filters, selected, context, selectedEntityValue, load, clearWorkspace, promoteToCase, navigate, fetchResults, applyFilters]);

  usePublishPageCommands(pageCommands);

  return (
    <div className="page-grid triage-page">
      <DashboardToolbar
        title="Signal workspace"
        subtitle="Native ClickHouse hunting surface with bounded windows, event-family pivots, and context panes that behave like a serious Grafana or Loki workflow."
        loading={loading}
        onRefresh={() => void (hasActiveQuery ? fetchResults(activeQueryParams) : load())}
        refreshButtonLabel={hasActiveQuery ? "Refresh current query" : "Run search"}
        className="triage-toolbar"
        actions={
          <div className="toolbar-inline-actions">
            <button
              type="button"
              className="secondary"
              onClick={() => {
                clearWorkspace();
                navigate("/events");
              }}
            >
              Clear workspace
            </button>
            <Link className="tool-btn secondary" to="/alerts">
              Alert inbox
            </Link>
            <Link className="tool-btn secondary" to="/detections">
              Detection ops
            </Link>
          </div>
        }
      >
        <form className="triage-filterbar triage-filterbar-wide" onSubmit={load}>
          <label>
            Analyst
            <input value={actor} onChange={(e) => setActor(e.target.value)} />
          </label>
          <label>
            Query
            <input
              value={filters.q}
              onChange={(e) => setFilters((p) => ({ ...p, q: e.target.value }))}
              placeholder="message / action / path / source"
            />
          </label>
          <label>
            Severity
            <select value={filters.severity} onChange={(e) => setFilters((p) => ({ ...p, severity: e.target.value }))}>
              <option value="">All</option>
              <option value="critical">critical</option>
              <option value="error">error</option>
              <option value="warning">warning</option>
              <option value="info">info</option>
            </select>
          </label>
          <label>
            Source type
            <input value={filters.source_type} onChange={(e) => setFilters((p) => ({ ...p, source_type: e.target.value }))} />
          </label>
          <label>
            Host
            <input value={filters.host} onChange={(e) => setFilters((p) => ({ ...p, host: e.target.value }))} />
          </label>
          <label>
            Source IP
            <input value={filters.source_ip} onChange={(e) => setFilters((p) => ({ ...p, source_ip: e.target.value }))} />
          </label>
          <label>
            User ID
            <input value={filters.user_id} onChange={(e) => setFilters((p) => ({ ...p, user_id: e.target.value }))} />
          </label>
          <label>
            Start
            <input value={filters.start} onChange={(e) => setFilters((p) => ({ ...p, start: e.target.value }))} placeholder="2026-04-14T09:00:00Z" />
          </label>
          <label>
            End
            <input value={filters.end} onChange={(e) => setFilters((p) => ({ ...p, end: e.target.value }))} placeholder="2026-04-14T10:00:00Z" />
          </label>
          <button type="submit">{loading ? "Searching..." : "Search"}</button>
        </form>
        <div className="time-preset-row">
          {TIME_PRESETS.map((preset) => (
            <button
              key={preset.id}
              type="button"
              className="secondary compact"
              onClick={() => applyFilters(applyRelativeWindow(preset.minutes))}
            >
              {preset.label}
            </button>
          ))}
        </div>
        {!!activeFilterChips.length && (
          <div className="toolbar-chip-row">
            {activeFilterChips.map((chip) => (
              <button
                key={chip.key}
                type="button"
                className="token token-action"
                onClick={() => applyFilters({ [chip.key]: "" } as Partial<Filters>)}
              >
                {chip.label}:{chip.value} x
              </button>
            ))}
          </div>
        )}
        <div className="toolbar-status-row">
          <span>Workspace window</span>
          <strong>{results ? formatFilterWindow(results.meta.filters.start, results.meta.filters.end) : "No committed query"}</strong>
          <span>Focused entity</span>
          <strong>{selectedEntityValue || "Awaiting selection"}</strong>
        </div>
        {results ? (
          <div className="summary-grid">
            <div className="summary-card">
              <span>Log lines</span>
              <strong>{formatCompact(results.meta.returned)}</strong>
            </div>
            <div className="summary-card">
              <span>Limit</span>
              <strong>{formatCompact(results.meta.limit)}</strong>
            </div>
            <div className="summary-card">
              <span>Top source</span>
              <strong>{topSourceTypes[0]?.[0] ?? "—"}</strong>
            </div>
            <div className="summary-card">
              <span>Time window</span>
              <strong>{formatFilterWindow(results.meta.filters.start, results.meta.filters.end)}</strong>
            </div>
            <div className="summary-card">
              <span>Critical + error</span>
              <strong>{formatCompact(severityCounts.critical + severityCounts.error)}</strong>
            </div>
            <div className="summary-card">
              <span>Top event type</span>
              <strong>{topEventTypes[0]?.[0] ?? "—"}</strong>
            </div>
          </div>
        ) : (
          <div className="surface-empty-state">
            <h3>No search committed yet</h3>
            <p>Set a bounded window or a focused entity, then run the query to populate the stream, context, and quick pivots.</p>
          </div>
        )}
        {err && <p className="error">{err}</p>}
      </DashboardToolbar>

      {!!logRows.length && (
        <section className="observability-grid observability-grid-primary">
          <ObservabilityLinePanel
            title="Signal volume strip"
            subtitle="Returned event density across the active result window"
            categories={streamLabels}
            series={[
              {
                name: "events",
                color: "#4d9bff",
                data: logRows.map(() => 1),
                areaOpacity: 0.16,
              },
            ]}
            axisFormatter={(value) => formatCompact(value)}
            valueFormatter={(value) => formatCompact(value)}
            kicker="Volume pane"
            showDataZoom
            onPointClick={({ dataIndex }) => {
              const row = logRows[dataIndex];
              if (row) {
                void openEvent(row);
              }
            }}
            footer={<p className="meta stat-subtle">Click the strip to jump into a log row, or zoom the current hunt window when the stream gets dense.</p>}
          />
          <ObservabilityBarPanel
            title="Top event types"
            subtitle="Most common event families in the current result set"
            rows={topEventTypes.map(([label, value], index) => ({
              label,
              value,
              color: index === 0 ? "#8f6dff" : "#4d9bff",
            }))}
            valueFormatter={(value) => formatCompact(value)}
            axisFormatter={(value) => formatCompact(value)}
            kicker="Distribution pane"
            onRowClick={({ label }) => {
              applyFilters({ q: label });
            }}
            footer={<p className="meta stat-subtle">Click a bar to pivot the query toward that event family immediately.</p>}
          />
        </section>
      )}

      <AdaptivePaneLayout
        storageKey="events-log-explorer"
        defaultSizes={[0.52, 0.28, 0.2]}
        minSizes={[0.36, 0.24, 0.18]}
        className="log-explorer-layout"
      >
        <section className="card event-result-shell workspace-pane">
          <div className="workspace-pane-header">
            <div className="workspace-pane-copy">
              <span className="workspace-pane-kicker">Stream pane</span>
              <h2>Log stream</h2>
              <p className="workspace-pane-subtitle">Dense event feed with severity accents, key fields and a Grafana-like scan rhythm.</p>
            </div>
          </div>
          {!results ? (
            <div className="surface-empty-state">
              <h3>Search is ready</h3>
              <p>Run the first query to load stream rows, event detail, and correlated entity activity.</p>
            </div>
          ) : !results.rows.length ? (
            <div className="surface-empty-state">
              <h3>No events matched</h3>
              <p>The current filters produced an empty result set. Broaden the window, remove a chip, or pivot to a different entity.</p>
            </div>
          ) : (
            <div className="log-stream">
              {results.rows.map((row) => {
                const priority = priorityFromSeverity(row.severity);
                const isActive = selected?.event.event_id === row.event_id;
                return (
                  <button
                    key={row.event_id}
                    type="button"
                    className={[
                      "log-row",
                      `severity-${priority.tone}`,
                      isActive ? "active" : "",
                    ]
                      .filter(Boolean)
                      .join(" ")}
                    onClick={() => openEvent(row)}
                  >
                    <div className="log-row-gutter">
                      <span className={`priority-pill priority-${priority.tone}`}>{priority.label}</span>
                      <time>{shortDateTime(row.timestamp)}</time>
                      <span className="log-row-type">{row.event_type || "event"}</span>
                    </div>
                    <div className="log-row-body">
                      <div className="log-row-head">
                        <div className="log-row-title">
                          <strong>{row.source_type}</strong>
                          <small>{row.host || "unknown host"}</small>
                        </div>
                        <div className="queue-item-badges">
                          <span className={`badge sev-${severityTone(row.severity)}`}>{row.severity}</span>
                          {row.status_code ? <span className="token">HTTP {row.status_code}</span> : null}
                        </div>
                      </div>
                      <p className="log-row-message">{row.message}</p>
                      <div className="log-row-meta">
                        <button
                          type="button"
                          className="token token-action"
                          onClick={(event) => {
                            event.stopPropagation();
                            applyFilters({ source_type: row.source_type });
                          }}
                        >
                          source:{row.source_type}
                        </button>
                        {row.source_ip ? <span className="token">ip:{row.source_ip}</span> : null}
                        {row.user_id ? <span className="token">user:{row.user_id}</span> : null}
                        {row.action ? <span className="token">{row.action}</span> : null}
                        {row.url_path ? <span className="token">{row.url_path}</span> : null}
                        <span className="token fp">{row.event_id}</span>
                      </div>
                    </div>
                  </button>
                );
              })}
            </div>
          )}
        </section>

        <section className="card entity-stack workspace-pane">
          <div className="workspace-pane-header">
            <div className="workspace-pane-copy">
              <span className="workspace-pane-kicker">Detail pane</span>
              <h2>Event detail</h2>
              <p className="workspace-pane-subtitle">Focused record view with technical fields, geo enrichment and case promotion actions.</p>
            </div>
          </div>
          {!selected ? (
            <div className="surface-empty-state">
              <h3>No event selected</h3>
              <p>Open a row from the log stream to inspect the record, copy identifiers, and promote evidence into casework.</p>
            </div>
          ) : (
            <>
              <div className="dashboard-hero">
                <div>
                  <strong>{selected.event.source_type}</strong>
                  <p className="meta">
                    <code>{selected.event.event_id}</code>
                  </p>
                </div>
                <div className="queue-item-badges">
                  <span className={`priority-pill priority-${priorityFromSeverity(selected.event.severity).tone}`}>
                    {priorityFromSeverity(selected.event.severity).label}
                  </span>
                  <span className={`badge sev-${severityTone(selected.event.severity)}`}>{selected.event.severity}</span>
                </div>
              </div>

              <div className="summary-grid">
                <div className="summary-card">
                  <span>Timestamp</span>
                  <strong>{shortDateTime(selected.event.timestamp)}</strong>
                </div>
                <div className="summary-card">
                  <span>Host</span>
                  <strong>{selected.event.host || "—"}</strong>
                </div>
                <div className="summary-card">
                  <span>Ingested</span>
                  <strong>{shortDateTime(selected.ingest_ts)}</strong>
                </div>
                <div className="summary-card">
                  <span>Action</span>
                  <strong>{selected.event.action || "—"}</strong>
                </div>
                <div className="summary-card">
                  <span>Event type</span>
                  <strong>{selected.event.event_type || "—"}</strong>
                </div>
              </div>

              <div className="insight-panel">
                <strong>Message</strong>
                <p>{selected.event.message}</p>
              </div>

              <div className="property-grid">
                <div className="property-card">
                  <span>HTTP method</span>
                  <strong>{selected.http_method || "—"}</strong>
                </div>
                <div className="property-card">
                  <span>Status code</span>
                  <strong>{selected.event.status_code ?? "—"}</strong>
                </div>
                <div className="property-card">
                  <span>Duration</span>
                  <strong>{selected.duration_ms != null ? `${selected.duration_ms.toFixed(1)} ms` : "—"}</strong>
                </div>
                <div className="property-card">
                  <span>Source IP</span>
                  <strong>{selected.event.source_ip || "—"}</strong>
                </div>
                <div className="property-card">
                  <span>User</span>
                  <strong>{selected.event.user_id || "—"}</strong>
                </div>
                <div className="property-card">
                  <span>Geo</span>
                  <strong>{selected.enrich.geo_country_name || selected.enrich.geo_country_iso || "—"}</strong>
                  <small>{selected.enrich.geo_city || selected.enrich.geo_org || ""}</small>
                </div>
              </div>

              <div className="dense-inline-actions">
                <button type="button" onClick={() => promoteToCase(selected.event)}>
                  Promote to case
                </button>
                <button type="button" className="secondary" onClick={() => navigator.clipboard.writeText(selected.event.event_id)}>
                  Copy event id
                </button>
                {selected.event.source_ip ? (
                  <button type="button" className="secondary" onClick={() => applyFilters({ source_ip: selected.event.source_ip || "" })}>
                    Filter by IP
                  </button>
                ) : null}
                {selected.event.user_id ? (
                  <button type="button" className="secondary" onClick={() => applyFilters({ user_id: selected.event.user_id || "" })}>
                    Filter by user
                  </button>
                ) : null}
              </div>

              <div>
                <p className="meta">Metadata</p>
                <pre className="sql-block">{JSON.stringify(selected.metadata, null, 2)}</pre>
              </div>
            </>
          )}
        </section>

        <aside className="detail-side entity-stack">
          <section className="card entity-stack workspace-pane">
            <div className="workspace-pane-header">
              <div className="workspace-pane-copy">
                <span className="workspace-pane-kicker">Context pane</span>
                <h2>Entity context</h2>
                <p className="workspace-pane-subtitle">Correlated entity activity, recent matching log lines and quick operational pivots.</p>
              </div>
            </div>
            {!context ? (
              <div className="surface-empty-state">
                <h3>No entity context yet</h3>
                <p>Open an event with `source_ip`, `user_id`, or `host` to build a correlated context pane for the active hunt.</p>
              </div>
            ) : (
              <>
                <div className="summary-grid">
                  <div className="summary-card">
                    <span>Entity</span>
                    <strong>{context.entity.kind}</strong>
                  </div>
                  <div className="summary-card">
                    <span>Value</span>
                    <strong>{context.entity.value}</strong>
                  </div>
                  <div className="summary-card">
                    <span>Events 24h</span>
                    <strong>{formatCompact(context.metrics.total_events_24h)}</strong>
                  </div>
                  <div className="summary-card">
                    <span>Error events</span>
                    <strong>{formatCompact(context.metrics.error_events_24h)}</strong>
                  </div>
                </div>

                {!!context.metrics.top_hosts.length && (
                  <div>
                    <p className="meta">Top hosts</p>
                    <div className="fact-list">
                      {context.metrics.top_hosts.map((host) => (
                        <span key={host} className="token">
                          {host}
                        </span>
                      ))}
                    </div>
                  </div>
                )}

                <div>
                  <p className="meta">Recent logs</p>
                  <div className="queue-list queue-list-dense">
                    {context.recent_events.slice(0, 6).map((row) => {
                      const priority = priorityFromSeverity(row.severity);
                      return (
                        <button
                          type="button"
                          key={row.event_id}
                          className={`queue-item queue-item-enterprise severity-${priority.tone}`}
                          onClick={() => openEvent(row)}
                        >
                          <header>
                            <div>
                              <h4>{row.source_type}</h4>
                              <p className="meta">{row.message}</p>
                            </div>
                            <div className="queue-item-badges">
                              <span className={`priority-pill priority-${priority.tone}`}>{priority.label}</span>
                              <span className={`badge sev-${severityTone(row.severity)}`}>{row.severity}</span>
                            </div>
                          </header>
                          <div className="queue-item-meta">
                            <span className="token">{shortDateTime(row.timestamp)}</span>
                            <span className="token">{row.host}</span>
                          </div>
                        </button>
                      );
                    })}
                  </div>
                </div>
              </>
            )}
          </section>

          <section className="card entity-stack workspace-pane">
            <div className="workspace-pane-header">
              <div className="workspace-pane-copy">
                <span className="workspace-pane-kicker">Action pane</span>
                <h2>Quick pivots</h2>
                <p className="workspace-pane-subtitle">Jump across cases, alerts and detection operations without leaving the log explorer.</p>
              </div>
            </div>
            <div className="dense-inline-actions">
              {selectedEntityValue ? (
                <Link className="tool-btn secondary inline" to={`/cases?q=${encodeURIComponent(selectedEntityValue)}`}>
                  Search cases
                </Link>
              ) : null}
              <Link className="tool-btn secondary inline" to="/alerts">
                Alert inbox
              </Link>
              <Link className="tool-btn secondary inline" to="/detections">
                Detection ops
              </Link>
              {selectedEntityValue ? (
                <Link className="tool-btn secondary inline" to={`/events?q=${encodeURIComponent(selectedEntityValue)}`}>
                  Re-run focused search
                </Link>
              ) : null}
            </div>
          </section>
        </aside>
      </AdaptivePaneLayout>
    </div>
  );
}
