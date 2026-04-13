import { useEffect, useMemo, useState } from "react";
import { Link, useSearchParams } from "react-router-dom";
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
import { useActorState } from "../components/PageLayout";
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

export default function EventsPage() {
  const [searchParams] = useSearchParams();
  const [filters, setFilters] = useState<Filters>(INITIAL_FILTERS);
  const [results, setResults] = useState<EventSearchResponse | null>(null);
  const [selected, setSelected] = useState<EventDetail | null>(null);
  const [context, setContext] = useState<EntityContext | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const { actor, setActor } = useActorState();

  const queryParams = useMemo(() => {
    const out: Record<string, string> = {};
    for (const [key, value] of Object.entries(filters)) {
      if (value.trim()) out[key] = value.trim();
    }
    return out;
  }, [filters]);

  useEffect(() => {
    const next = { ...INITIAL_FILTERS };
    for (const key of Object.keys(next) as Array<keyof Filters>) {
      const value = searchParams.get(key);
      if (value) next[key] = value;
    }
    setFilters(next);
  }, [searchParams]);

  const load = async (e?: React.FormEvent) => {
    e?.preventDefault();
    setLoading(true);
    setErr(null);
    try {
      const data = await searchEvents(queryParams);
      setResults(data);
      setSelected(null);
      setContext(null);
    } catch (error) {
      setErr(String(error));
    } finally {
      setLoading(false);
    }
  };

  useEffect(() => {
    if (!Object.keys(queryParams).length) return;
    setLoading(true);
    setErr(null);
    searchEvents(queryParams)
      .then((data) => {
        setResults(data);
        setSelected(null);
        setContext(null);
      })
      .catch((error) => setErr(String(error)))
      .finally(() => setLoading(false));
  }, [queryParams]);

  const openEvent = async (row: EventRow) => {
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
  };

  const selectedEntityValue = selected?.event.source_ip || selected?.event.user_id || selected?.event.host;

  const promoteToCase = async (row: EventRow) => {
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
      window.location.href = `/cases/${created.id}`;
    } catch (error) {
      setErr(String(error));
    }
  };

  return (
    <div className="page-grid">
      <section className="card hero-card">
        <h2>Native event search</h2>
        <p className="meta">
          Safe read-only поиск по ClickHouse через портал. Теперь с более взрослым detail pane, entity context и удобными pivots.
        </p>
        <form className="toolbar" onSubmit={load}>
          <label>
            Analyst
            <input value={actor} onChange={(e) => setActor(e.target.value)} />
          </label>
          <label>
            Query
            <input value={filters.q} onChange={(e) => setFilters((p) => ({ ...p, q: e.target.value }))} placeholder="message / action / path" />
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
          <button type="submit">{loading ? "Searching…" : "Search"}</button>
        </form>
        {results && (
          <div className="summary-grid">
            <div className="summary-card">
              <span>Returned</span>
              <strong>{formatCompact(results.meta.returned)}</strong>
            </div>
            <div className="summary-card">
              <span>Limit</span>
              <strong>{formatCompact(results.meta.limit)}</strong>
            </div>
            <div className="summary-card">
              <span>Window start</span>
              <strong>{results.meta.filters.start}</strong>
            </div>
            <div className="summary-card">
              <span>Window end</span>
              <strong>{results.meta.filters.end}</strong>
            </div>
          </div>
        )}
        {err && <p className="error">{err}</p>}
      </section>

      <section className="entity-layout">
        <div className="entity-stack">
          <section className="card event-result-shell">
            <h2>Search results</h2>
            {!results ? (
              <p className="meta">Нажми Search, чтобы загрузить события.</p>
            ) : !results.rows.length ? (
              <p className="meta">По текущим фильтрам ничего не найдено.</p>
            ) : (
              <div className="event-table-shell">
                <table className="compact-table">
                  <thead>
                    <tr>
                      <th>Time</th>
                      <th>Severity</th>
                      <th>Source</th>
                      <th>Host</th>
                      <th>Message</th>
                    </tr>
                  </thead>
                  <tbody>
                    {results.rows.map((row) => (
                      <tr
                        key={row.event_id}
                        onClick={() => openEvent(row)}
                        className={selected?.event.event_id === row.event_id ? "selectable-row active" : "selectable-row"}
                      >
                        <td>{shortDateTime(row.timestamp)}</td>
                        <td>
                          <span className={`badge sev-${row.severity.toLowerCase()}`}>{row.severity}</span>
                        </td>
                        <td>{row.source_type}</td>
                        <td>{row.host}</td>
                        <td>
                          <div>{row.message}</div>
                          <div className="fact-list" style={{ marginTop: "0.45rem" }}>
                            {row.source_ip ? <span className="token">ip:{row.source_ip}</span> : null}
                            {row.user_id ? <span className="token">user:{row.user_id}</span> : null}
                            {row.action ? <span className="token">{row.action}</span> : null}
                            {row.url_path ? <span className="token">{row.url_path}</span> : null}
                          </div>
                        </td>
                      </tr>
                    ))}
                  </tbody>
                </table>
              </div>
            )}
          </section>

          {selected && (
            <section className="card entity-stack">
              <div className="dashboard-hero">
                <div>
                  <h2>Event detail</h2>
                  <p className="meta">
                    <code>{selected.event.event_id}</code>
                  </p>
                </div>
                <span className={`badge sev-${selected.event.severity.toLowerCase()}`}>{selected.event.severity}</span>
              </div>

              <div className="summary-grid">
                <div className="summary-card">
                  <span>Timestamp</span>
                  <strong>{shortDateTime(selected.event.timestamp)}</strong>
                </div>
                <div className="summary-card">
                  <span>Source type</span>
                  <strong>{selected.event.source_type}</strong>
                </div>
                <div className="summary-card">
                  <span>Host</span>
                  <strong>{selected.event.host || "—"}</strong>
                </div>
                <div className="summary-card">
                  <span>Ingested</span>
                  <strong>{shortDateTime(selected.ingest_ts)}</strong>
                </div>
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
                  <span>Action</span>
                  <strong>{selected.event.action || "—"}</strong>
                </div>
                <div className="property-card">
                  <span>Source IP</span>
                  <strong>{selected.event.source_ip || "—"}</strong>
                </div>
                <div className="property-card">
                  <span>User</span>
                  <strong>{selected.event.user_id || "—"}</strong>
                </div>
              </div>

              <div className="card" style={{ padding: "0.9rem 1rem" }}>
                <span className="meta">Message</span>
                <p style={{ margin: "0.35rem 0 0" }}>{selected.event.message}</p>
              </div>

              <div className="property-grid">
                <div className="property-card">
                  <span>Geo country</span>
                  <strong>{selected.enrich.geo_country_name || selected.enrich.geo_country_iso || "—"}</strong>
                </div>
                <div className="property-card">
                  <span>Geo city</span>
                  <strong>{selected.enrich.geo_city || "—"}</strong>
                </div>
                <div className="property-card">
                  <span>ASN</span>
                  <strong>{selected.enrich.geo_asn ?? "—"}</strong>
                </div>
                <div className="property-card">
                  <span>Org</span>
                  <strong>{selected.enrich.geo_org || "—"}</strong>
                </div>
              </div>

              <div className="dense-inline-actions">
                <button type="button" onClick={() => promoteToCase(selected.event)}>
                  Promote to case
                </button>
                {selected.event.source_ip ? (
                  <button type="button" className="secondary" onClick={() => setFilters((p) => ({ ...p, source_ip: selected.event.source_ip || "" }))}>
                    Filter by IP
                  </button>
                ) : null}
                {selected.event.user_id ? (
                  <button type="button" className="secondary" onClick={() => setFilters((p) => ({ ...p, user_id: selected.event.user_id || "" }))}>
                    Filter by user
                  </button>
                ) : null}
              </div>

              <div>
                <p className="meta">Metadata</p>
                <pre className="sql-block">{JSON.stringify(selected.metadata, null, 2)}</pre>
              </div>
            </section>
          )}
        </div>

        <aside className="detail-side entity-stack">
          <section className="card entity-stack">
            <h2>Entity context</h2>
            {!context ? (
              <p className="meta">Открой событие с `source_ip`, `user_id` или `host`, чтобы получить контекст.</p>
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
                  <p className="meta">Recent events</p>
                  <div className="queue-list">
                    {context.recent_events.slice(0, 8).map((row) => (
                      <button
                        type="button"
                        key={row.event_id}
                        className="queue-item"
                        onClick={() => openEvent(row)}
                      >
                        <header>
                          <div>
                            <h4>{row.source_type}</h4>
                            <p className="meta">{row.message}</p>
                          </div>
                          <span className={`badge sev-${row.severity.toLowerCase()}`}>{row.severity}</span>
                        </header>
                        <div className="queue-item-meta">
                          <span className="token">{shortDateTime(row.timestamp)}</span>
                          <span className="token">{row.host}</span>
                        </div>
                      </button>
                    ))}
                  </div>
                </div>
              </>
            )}
          </section>

          <section className="card entity-stack">
            <h2>Quick pivots</h2>
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
            </div>
          </section>
        </aside>
      </section>
    </div>
  );
}
