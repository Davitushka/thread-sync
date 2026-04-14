import { useEffect, useMemo, useState } from "react";
import { Link, useNavigate, useParams } from "react-router-dom";
import { getCase, getInvestigation, type CaseDetail, type Investigation, type LinkedAlert, type LinkedEvent, type TimelineEntry } from "../api";
import DashboardToolbar from "../components/DashboardToolbar";
import { usePublishPageCommands, type SuitePageCommand } from "../components/SuiteCommandContext";
import { useWorkspaceShell } from "../components/WorkspaceShellContext";
import { formatCompact, shortDateTime } from "../dashboard-utils";

function sevClass(s: string) {
  return `badge sev-${s}`;
}

function exploreClickhouseUrl(grafanaRoot: string, sql: string): string {
  const base = grafanaRoot.replace(/\/$/, "");
  const panes = {
    siem: {
      datasource: "clickhouse-siem",
      queries: [{ refId: "A", queryType: "sql", rawSql: sql }],
    },
  };
  return `${base}/explore?schemaVersion=1&panes=${encodeURIComponent(JSON.stringify(panes))}`;
}

type FeedItem =
  | { kind: "timeline"; ts: string; entry: TimelineEntry }
  | { kind: "alert"; ts: string; alert: LinkedAlert }
  | { kind: "event"; ts: string; event: LinkedEvent };

function buildFeed(data: CaseDetail): FeedItem[] {
  const out: FeedItem[] = [];
  for (const t of data.timeline) out.push({ kind: "timeline", ts: t.created_at, entry: t });
  for (const a of data.linked_alerts) out.push({ kind: "alert", ts: a.last_seen_at, alert: a });
  for (const e of data.linked_events) out.push({ kind: "event", ts: e.linked_at, event: e });
  out.sort((a, b) => new Date(b.ts).getTime() - new Date(a.ts).getTime());
  return out;
}

function grafanaOrigin(inv: Investigation | null): string {
  const u = inv?.grafana?.siem_overview;
  if (!u) return "http://localhost:3000";
  try {
    return new URL(u).origin;
  } catch {
    return "http://localhost:3000";
  }
}

function formatSla(due?: string): string {
  if (!due) return "—";
  const d = new Date(due);
  const diff = d.getTime() - Date.now();
  if (diff < 0) return `overdue (${d.toLocaleString()})`;
  const h = Math.floor(diff / 3600000);
  const m = Math.floor((diff % 3600000) / 60000);
  return `due ${d.toLocaleString()} (~${h}h ${m}m)`;
}

async function copyText(text: string) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    window.prompt("Copy value:", text);
  }
}

export default function InvestigationWorkbench() {
  const { id } = useParams<{ id: string }>();
  const navigate = useNavigate();
  const { updateWorkspaceMeta } = useWorkspaceShell();
  const [data, setData] = useState<CaseDetail | null>(null);
  const [inv, setInv] = useState<Investigation | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [invErr, setInvErr] = useState<string | null>(null);

  useEffect(() => {
    if (!id) return;
    setErr(null);
    setInvErr(null);
    getCase(id)
      .then(setData)
      .catch((e) => {
        setErr(String(e));
        setData(null);
      });
    getInvestigation(id)
      .then(setInv)
      .catch((e) => setInvErr(String(e)));
  }, [id]);

  const feed = useMemo(() => (data ? buildFeed(data) : []), [data]);
  const grafanaBase = useMemo(() => grafanaOrigin(inv), [inv]);

  useEffect(() => {
    if (!id || !data) return;
    updateWorkspaceMeta(`/cases/${id}/investigate`, {
      label: `${data.display_key} investigation`,
      tabLabel: `${data.display_key} Investigate`,
      title: `${data.display_key} - Investigation`,
      subtitle: data.title || "Investigation workbench",
      keywords: `${data.display_key} ${data.title} investigation`,
    });
  }, [data, id, updateWorkspaceMeta]);

  const pageCommands = useMemo<SuitePageCommand[]>(() => {
    if (!data) return [];

    const commands: SuitePageCommand[] = [
      {
        id: `investigation:case:${data.id}`,
        title: `Open ${data.display_key} case detail`,
        subtitle: "Return to the structured case workspace from investigation mode.",
        section: "Current investigation",
        keywords: `${data.display_key} case detail`,
        priority: 92,
        run: () => navigate(`/cases/${data.id}`),
      },
      {
        id: `investigation:copy:${data.id}`,
        title: `Copy ${data.display_key}`,
        subtitle: "Copy the active case display key to the clipboard.",
        section: "Current investigation",
        keywords: `${data.display_key} copy`,
        priority: 70,
        run: () => navigator.clipboard.writeText(data.display_key),
      },
    ];

    if (inv?.runbook_url) {
      commands.push({
        id: `investigation:runbook:${data.id}`,
        title: "Open attached runbook",
        subtitle: "Open the investigation runbook in a separate tab.",
        section: "Current investigation",
        keywords: `${data.display_key} runbook`,
        priority: 95,
        href: inv.runbook_url,
        external: true,
      });
    }
    if (inv?.grafana?.siem_overview) {
      commands.push({
        id: `investigation:grafana:${data.id}`,
        title: "Open SIEM dashboard",
        subtitle: "Open the linked investigation Grafana overview in a separate tab.",
        section: "Current investigation",
        keywords: `${data.display_key} grafana siem dashboard`,
        priority: 88,
        href: inv.grafana.siem_overview,
        external: true,
      });
    }
    if (inv?.suggested_clickhouse_queries?.[0]?.sql) {
      commands.push({
        id: `investigation:copy-sql:${data.id}`,
        title: `Copy SQL: ${inv.suggested_clickhouse_queries[0].title}`,
        subtitle: "Copy the first suggested ClickHouse query for quick analyst pivots.",
        section: "ClickHouse pivots",
        keywords: `${inv.suggested_clickhouse_queries[0].title} sql clickhouse`,
        priority: 86,
        run: () => void copyText(inv.suggested_clickhouse_queries[0].sql),
      });
    }
    if (data.linked_alerts[0]?.fingerprint) {
      commands.push({
        id: `investigation:alert-fp:${data.id}`,
        title: "Copy first linked alert fingerprint",
        subtitle: "Copy the first linked alert fingerprint from this investigation.",
        section: "Alert context",
        keywords: `${data.linked_alerts[0].fingerprint} alert fingerprint`,
        priority: 74,
        run: () => navigator.clipboard.writeText(data.linked_alerts[0].fingerprint),
      });
    }
    if (data.linked_events[0]?.event_id) {
      commands.push({
        id: `investigation:event:${data.id}`,
        title: "Search first linked event",
        subtitle: "Pivot into native event search using the first linked event identifier.",
        section: "Alert context",
        keywords: `${data.linked_events[0].event_id} event search`,
        priority: 82,
        run: () => navigate(`/events?q=${encodeURIComponent(data.linked_events[0].event_id)}`),
      });
    }

    return commands;
  }, [data, inv, navigate]);

  usePublishPageCommands(pageCommands);

  if (!id) return <p>Invalid investigation URL.</p>;
  if (err && !data) return <p className="error">{err}</p>;
  if (!data) return <p className="meta">Loading investigation workspace...</p>;

  const g = inv?.grafana;

  return (
    <div className="page-grid casework-page workbench">
      <DashboardToolbar
        title={`${data.display_key} - Investigation`}
        subtitle="Unified investigation workspace with a merged evidence feed, analyst pivots, and guided movement into external deep-dive tools."
        className="casework-toolbar"
        actions={
          <div className="toolbar-inline-actions">
            <Link className="tool-btn secondary" to={`/cases/${id}`}>
              Open case detail
            </Link>
          </div>
        }
      >
        <div className="case-hero-title">
          <div className="case-hero-crumbs meta">
            <Link to="/cases">Cases</Link>
            <span>/</span>
            <Link to={`/cases/${id}`}>{data.display_key}</Link>
            <span>/</span>
            <span>Investigation</span>
          </div>
          <div className="workbench-kpis">
            <span className={sevClass(data.severity)}>{data.severity}</span>
            <span className="kpi-pill">{data.status}</span>
            {data.assignee && <span className="kpi-pill">@{data.assignee}</span>}
            <span className="kpi-pill">SLA: {formatSla(data.due_at)}</span>
          </div>
        </div>
        <div className="summary-grid">
          <div className="summary-card">
            <span>Linked alerts</span>
            <strong>{formatCompact(data.linked_alerts.length)}</strong>
          </div>
          <div className="summary-card">
            <span>Linked events</span>
            <strong>{formatCompact(data.linked_events.length)}</strong>
          </div>
          <div className="summary-card">
            <span>Timeline entries</span>
            <strong>{formatCompact(data.timeline.length)}</strong>
          </div>
          <div className="summary-card">
            <span>Runbook</span>
            <strong>{inv?.runbook_url ? "attached" : "—"}</strong>
          </div>
        </div>
      </DashboardToolbar>

      {invErr && <p className="error workbench-banner">Investigation summary unavailable: {invErr}</p>}

      <div className="workbench-grid">
        <div className="entity-stack">
          <section className="card workbench-actions entity-stack">
            <h2>Investigation controls</h2>
            <div className="property-grid">
              <div className="property-card">
                <span>Status workflow</span>
                <strong>{inv?.process.status_workflow.join(" -> ") || "—"}</strong>
              </div>
              <div className="property-card">
                <span>SLA hint</span>
                <strong>{inv?.process.sla_hint || "—"}</strong>
              </div>
              <div className="property-card">
                <span>Acknowledged</span>
                <strong>{data.acknowledged_at ? shortDateTime(data.acknowledged_at) : "—"}</strong>
              </div>
              <div className="property-card">
                <span>Due at</span>
                <strong>{data.due_at ? shortDateTime(data.due_at) : "—"}</strong>
              </div>
            </div>

            <div className="dense-inline-actions">
              {g?.siem_overview && (
                <a className="tool-btn" href={g.siem_overview} target="_blank" rel="noreferrer">
                  SIEM dashboard
                </a>
              )}
              {g?.explore_clickhouse_preset && (
                <a className="tool-btn" href={g.explore_clickhouse_preset} target="_blank" rel="noreferrer">
                  Explore preset
                </a>
              )}
              {g?.explore_loki && (
                <a className="tool-btn secondary" href={g.explore_loki} target="_blank" rel="noreferrer">
                  Explore Loki
                </a>
              )}
              {inv?.runbook_url && (
                <a className="tool-btn secondary" href={inv.runbook_url} target="_blank" rel="noreferrer">
                  Runbook
                </a>
              )}
              {g?.data_quality_dashboard && (
                <a className="tool-btn secondary" href={g.data_quality_dashboard} target="_blank" rel="noreferrer">
                  Data quality
                </a>
              )}
            </div>
          </section>

          <section className="card workbench-feed">
            <h2>Unified feed</h2>
            {feed.length === 0 ? (
              <div className="surface-empty-state">
                <h3>No investigation feed yet</h3>
                <p>Timeline entries, linked alerts, and linked events will accumulate here as the case advances.</p>
              </div>
            ) : (
              <ul className="feed-list">
                {feed.map((item, idx) => (
                  <li key={`${item.kind}-${idx}-${item.ts}`} className={`feed-item feed-${item.kind}`}>
                    <time>{shortDateTime(item.ts)}</time>
                    {item.kind === "timeline" && (
                      <div>
                        <strong>{item.entry.actor}</strong> · <span className="feed-tag">{item.entry.entry_type}</span>
                        {item.entry.body && <p className="feed-body">{item.entry.body}</p>}
                      </div>
                    )}
                    {item.kind === "alert" && (
                      <div>
                        <span className="feed-tag">Alert</span> <code className="fp">{item.alert.fingerprint.slice(0, 20)}…</code>
                        <div>
                          <strong>{item.alert.rule_title ?? item.alert.rule_id ?? "rule"}</strong>
                          {item.alert.severity && <span className={sevClass(item.alert.severity)}>{item.alert.severity}</span>}
                        </div>
                        {item.alert.description && <p className="feed-body">{item.alert.description}</p>}
                      </div>
                    )}
                    {item.kind === "event" && (
                      <div>
                        <span className="feed-tag">Event</span> <code>{item.event.event_id}</code>
                        {item.event.note && <p className="feed-body">{item.event.note}</p>}
                      </div>
                    )}
                  </li>
                ))}
              </ul>
            )}
          </section>
        </div>

        <aside className="workbench-aside">
          <div className="card entity-stack">
            <h2>Alert context</h2>
            {data.linked_alerts.length === 0 ? (
              <div className="surface-empty-state">
                <h3>No linked alerts</h3>
                <p>Attach alerts from the case workspace when you need rule context, label evidence, or response history here.</p>
              </div>
            ) : (
              <div className="queue-list">
                {data.linked_alerts.map((a) => (
                  <article key={a.fingerprint} className="queue-item">
                    <header>
                      <div>
                        <h4>{a.rule_title ?? a.rule_id ?? "Alert"}</h4>
                        <p className="meta fp-wrap">
                          <code>{a.fingerprint}</code>
                        </p>
                      </div>
                      <span className={a.severity ? sevClass(a.severity) : "badge"}>{a.severity ?? "—"}</span>
                    </header>
                    {a.description && <p className="alert-desc">{a.description}</p>}
                    {a.context && Object.keys(a.context).length > 0 && <pre className="context-json">{JSON.stringify(a.context, null, 2)}</pre>}
                  </article>
                ))}
              </div>
            )}
          </div>

          <div className="card entity-stack">
            <h2>ClickHouse pivots</h2>
            {!inv?.suggested_clickhouse_queries?.length ? (
              <div className="surface-empty-state">
                <h3>No suggested pivots</h3>
                <p>Suggested ClickHouse queries will appear here when the investigation summary returns evidence-driven pivots.</p>
              </div>
            ) : (
              <div className="query-card-list">
                {inv.suggested_clickhouse_queries.map((q, i) => (
                  <article key={i} className="query-card">
                    <h3>{q.title}</h3>
                    <pre className="sql-block">{q.sql}</pre>
                    <div className="btn-row tight">
                      <button type="button" className="secondary" onClick={() => copyText(q.sql)}>
                        Copy SQL
                      </button>
                      <a className="tool-btn inline" href={exploreClickhouseUrl(grafanaBase, q.sql)} target="_blank" rel="noreferrer">
                        Explore
                      </a>
                    </div>
                  </article>
                ))}
              </div>
            )}
          </div>
        </aside>
      </div>
    </div>
  );
}
