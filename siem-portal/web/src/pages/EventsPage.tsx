import { useMemo, useState } from "react";
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
  const [filters, setFilters] = useState<Filters>(INITIAL_FILTERS);
  const [results, setResults] = useState<EventSearchResponse | null>(null);
  const [selected, setSelected] = useState<EventDetail | null>(null);
  const [context, setContext] = useState<EntityContext | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [loading, setLoading] = useState(false);
  const [actor, setActor] = useState(() => localStorage.getItem("soc_actor") || "analyst");

  const queryParams = useMemo(() => {
    const out: Record<string, string> = {};
    for (const [key, value] of Object.entries(filters)) {
      if (value.trim()) out[key] = value.trim();
    }
    return out;
  }, [filters]);

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

  const promoteToCase = async (row: EventRow) => {
    setErr(null);
    localStorage.setItem("soc_actor", actor);
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
      <section className="card">
        <h2>Native event search</h2>
        <p className="meta">
          Safe read-only поиск по ClickHouse через портал. По умолчанию — последние 24 часа, без raw SQL в браузере.
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
        {err && <p className="error">{err}</p>}
        {!results ? (
          <p className="meta">Нажми Search, чтобы загрузить события.</p>
        ) : (
          <>
            <p className="meta">
              Returned: {results.meta.returned} / limit {results.meta.limit}
            </p>
            <table>
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
                  <tr key={row.event_id} onClick={() => openEvent(row)} style={{ cursor: "pointer" }}>
                    <td>{new Date(row.timestamp).toLocaleString()}</td>
                    <td>
                      <span className={`badge sev-${row.severity.toLowerCase()}`}>{row.severity}</span>
                    </td>
                    <td>{row.source_type}</td>
                    <td>{row.host}</td>
                    <td>
                      <div>{row.message}</div>
                      <div className="btn-row tight" onClick={(e) => e.stopPropagation()}>
                        <button type="button" className="secondary" onClick={() => promoteToCase(row)}>
                          Create case
                        </button>
                      </div>
                    </td>
                  </tr>
                ))}
              </tbody>
            </table>
          </>
        )}
      </section>

      {(selected || context) && (
        <section className="detail-grid">
          {selected && (
            <div className="card">
              <h2>Event detail</h2>
              <p className="meta">
                <code>{selected.event.event_id}</code>
              </p>
              <pre className="sql-block">{JSON.stringify(selected, null, 2)}</pre>
            </div>
          )}
          {context && (
            <div className="card">
              <h2>Entity context</h2>
              <p className="meta">
                {context.entity.kind}: <code>{context.entity.value}</code>
              </p>
              <p className="meta">
                Events 24h: {context.metrics.total_events_24h} · Errors: {context.metrics.error_events_24h}
              </p>
              <ul className="event-list">
                {context.recent_events.map((row) => (
                  <li key={row.event_id}>
                    <code>{row.event_id}</code> · {row.message}
                  </li>
                ))}
              </ul>
            </div>
          )}
        </section>
      )}
    </div>
  );
}
