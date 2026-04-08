import { useEffect, useMemo, useState } from "react";
import { Link, useParams } from "react-router-dom";
import {
  getCase,
  getInvestigation,
  type CaseDetail,
  type Investigation,
  type LinkedAlert,
  type LinkedEvent,
  type TimelineEntry,
} from "../api";

function sevClass(s: string) {
  return `badge sev-${s}`;
}

/** Explore → ClickHouse (provisioning datasource id `clickhouse-siem`). */
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
  for (const t of data.timeline) {
    out.push({ kind: "timeline", ts: t.created_at, entry: t });
  }
  for (const a of data.linked_alerts) {
    out.push({ kind: "alert", ts: a.last_seen_at, alert: a });
  }
  for (const e of data.linked_events) {
    out.push({ kind: "event", ts: e.linked_at, event: e });
  }
  out.sort((a, b) => new Date(b.ts).getTime() - new Date(a.ts).getTime());
  return out;
}

function grafanaOrigin(inv: Investigation | null): string {
  const u = inv?.grafana?.siem_overview;
  if (!u) return "http://localhost:3000";
  try {
    const url = new URL(u);
    return url.origin;
  } catch {
    return "http://localhost:3000";
  }
}

function formatSla(due?: string): string {
  if (!due) return "—";
  const d = new Date(due);
  const now = Date.now();
  const diff = d.getTime() - now;
  if (diff < 0) return `просрочено (${d.toLocaleString()})`;
  const h = Math.floor(diff / 3600000);
  const m = Math.floor((diff % 3600000) / 60000);
  return `до ${d.toLocaleString()} (~${h}ч ${m}м)`;
}

async function copyText(text: string) {
  try {
    await navigator.clipboard.writeText(text);
  } catch {
    window.prompt("Копирование:", text);
  }
}

export default function InvestigationWorkbench() {
  const { id } = useParams<{ id: string }>();
  const [data, setData] = useState<CaseDetail | null>(null);
  const [inv, setInv] = useState<Investigation | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [invErr, setInvErr] = useState<string | null>(null);

  const load = () => {
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
  };

  useEffect(() => {
    load();
  }, [id]);

  const feed = useMemo(() => (data ? buildFeed(data) : []), [data]);
  const grafanaBase = useMemo(() => grafanaOrigin(inv), [inv]);

  if (!id) return <p>Некорректный URL</p>;
  if (err && !data) return <p className="error">{err}</p>;
  if (!data) return <p className="meta">Загрузка расследования…</p>;

  const g = inv?.grafana;

  return (
    <div className="workbench">
      <p className="meta" style={{ marginBottom: "0.5rem" }}>
        <Link to="/">Главная</Link>
        {" · "}
        <Link to="/cases">Кейсы</Link>
        {" · "}
        <Link to={`/cases/${id}`}>Карточка</Link>
      </p>

      <header className="workbench-header">
        <div>
          <h1 style={{ margin: "0 0 0.35rem", fontSize: "1.35rem" }}>
            {data.display_key} — {data.title}
          </h1>
          <p className="meta" style={{ margin: 0 }}>
            Рабочее место аналитика · объединённая лента, алерты и pivot в данные
          </p>
        </div>
        <div className="workbench-kpis">
          <span className={sevClass(data.severity)}>{data.severity}</span>
          <span className="kpi-pill">{data.status}</span>
          {data.assignee && <span className="kpi-pill">@{data.assignee}</span>}
          <span className="kpi-pill" title="SLA по серверу">
            SLA: {formatSla(data.due_at)}
          </span>
          {data.acknowledged_at && (
            <span className="kpi-pill ok">Подтв. {new Date(data.acknowledged_at).toLocaleString()}</span>
          )}
        </div>
      </header>

      {invErr && <p className="error workbench-banner">Сводка расследования: {invErr}</p>}

      <section className="workbench-actions card">
        <h2>Данные и инструменты</h2>
        <div className="btn-row">
          {g?.siem_overview && (
            <a className="tool-btn" href={g.siem_overview} target="_blank" rel="noreferrer">
              Дашборд SIEM
            </a>
          )}
          {g?.explore_clickhouse_preset && (
            <a className="tool-btn" href={g.explore_clickhouse_preset} target="_blank" rel="noreferrer">
              Explore: события (preset)
            </a>
          )}
          {g?.explore_loki && (
            <a className="tool-btn" href={g.explore_loki} target="_blank" rel="noreferrer">
              Explore: Loki
            </a>
          )}
          {g?.data_quality_dashboard && (
            <a className="tool-btn secondary" href={g.data_quality_dashboard} target="_blank" rel="noreferrer">
              Качество данных
            </a>
          )}
          {(data.runbook_url || inv?.runbook_url) && (
            <a
              className="tool-btn secondary"
              href={(data.runbook_url || inv?.runbook_url)!}
              target="_blank"
              rel="noreferrer"
            >
              Runbook
            </a>
          )}
        </div>
        {inv?.process && (
          <p className="meta" style={{ marginBottom: 0, marginTop: "0.75rem" }}>
            Процесс: {inv.process.status_workflow.join(" → ")}. {inv.process.sla_hint}
          </p>
        )}
      </section>

      <div className="workbench-grid">
        <section className="card workbench-feed">
          <h2>Объединённая лента</h2>
          <p className="meta" style={{ marginTop: 0 }}>
            Комментарии, системные записи, привязанные алерты и события — по времени (новые сверху).
          </p>
          {feed.length === 0 ? (
            <p className="meta">Пока пусто. Добавьте комментарии или привязки на карточке кейса.</p>
          ) : (
            <ul className="feed-list">
              {feed.map((item, idx) => (
                <li key={`${item.kind}-${idx}-${item.ts}`} className={`feed-item feed-${item.kind}`}>
                  <time>{new Date(item.ts).toLocaleString()}</time>
                  {item.kind === "timeline" && (
                    <div>
                      <strong>{item.entry.actor}</strong> · <span className="feed-tag">{item.entry.entry_type}</span>
                      {item.entry.body && <p className="feed-body">{item.entry.body}</p>}
                    </div>
                  )}
                  {item.kind === "alert" && (
                    <div>
                      <span className="feed-tag">Алерт</span>{" "}
                      <code className="fp">{item.alert.fingerprint.slice(0, 20)}…</code>
                      <div>
                        <strong>{item.alert.rule_title ?? item.alert.rule_id ?? "правило"}</strong>
                        {item.alert.severity && (
                          <>
                            {" "}
                            <span className={sevClass(item.alert.severity)}>{item.alert.severity}</span>
                          </>
                        )}
                      </div>
                      {item.alert.description && <p className="feed-body">{item.alert.description}</p>}
                    </div>
                  )}
                  {item.kind === "event" && (
                    <div>
                      <span className="feed-tag">Событие</span> <code>{item.event.event_id}</code>
                      {item.event.note && <p className="feed-body">{item.event.note}</p>}
                    </div>
                  )}
                </li>
              ))}
            </ul>
          )}
        </section>

        <aside className="workbench-aside">
          <div className="card">
            <h2>Связанные алерты — контекст</h2>
            {data.linked_alerts.length === 0 ? (
              <p className="meta">Нет привязанных алертов.</p>
            ) : (
              <div className="alert-stack">
                {data.linked_alerts.map((a) => (
                  <article key={a.fingerprint} className="alert-card">
                    <header>
                      <span className={a.severity ? sevClass(a.severity) : "badge"}>{a.severity ?? "—"}</span>
                      <h3>{a.rule_title ?? a.rule_id ?? "Alert"}</h3>
                    </header>
                    <p className="meta fp-wrap">
                      <code>{a.fingerprint}</code>
                    </p>
                    {a.description && <p className="alert-desc">{a.description}</p>}
                    <p className="meta">
                      Первый раз: {new Date(a.first_seen_at).toLocaleString()} · Последний:{" "}
                      {new Date(a.last_seen_at).toLocaleString()}
                    </p>
                    {a.context && Object.keys(a.context).length > 0 && (
                      <pre className="context-json">{JSON.stringify(a.context, null, 2)}</pre>
                    )}
                  </article>
                ))}
              </div>
            )}
          </div>

          <div className="card">
            <h2>Pivot: ClickHouse</h2>
            <p className="meta" style={{ marginTop: 0 }}>
              Готовые запросы с бэкенда. Открыть в Grafana Explore или скопировать SQL.
            </p>
            {!inv?.suggested_clickhouse_queries?.length ? (
              <p className="meta">Нет предложений (загрузите сводку расследования или привяжите алерты с контекстом).</p>
            ) : (
              <ul className="query-suggest">
                {inv.suggested_clickhouse_queries.map((q, i) => (
                  <li key={i}>
                    <div className="query-title">{q.title}</div>
                    <pre className="sql-block">{q.sql}</pre>
                    <div className="btn-row tight">
                      <button type="button" className="secondary" onClick={() => copyText(q.sql)}>
                        Копировать SQL
                      </button>
                      <a
                        className="tool-btn inline"
                        href={exploreClickhouseUrl(grafanaBase, q.sql)}
                        target="_blank"
                        rel="noreferrer"
                      >
                        Explore
                      </a>
                    </div>
                  </li>
                ))}
              </ul>
            )}
          </div>

          <div className="card">
            <h2>Связанные события</h2>
            {data.linked_events.length === 0 ? (
              <p className="meta">Нет UUID событий. Добавьте на карточке кейса.</p>
            ) : (
              <ul className="event-list">
                {data.linked_events.map((e) => (
                  <li key={e.event_id}>
                    <code>{e.event_id}</code>
                    <span className="meta"> привязано {new Date(e.linked_at).toLocaleString()}</span>
                    {e.note && <div className="meta">{e.note}</div>}
                  </li>
                ))}
              </ul>
            )}
          </div>
        </aside>
      </div>
    </div>
  );
}
