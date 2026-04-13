import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import {
  getAlerts,
  getCorrelatorStats,
  getOverviewDashboard,
  listCases,
  stackStatus,
  uiConfig,
  type OverviewDashboard,
  type StackStatus,
  type UiConfig,
} from "../api";

function compact(n: number | null | undefined): string {
  if (n == null) return "—";
  return new Intl.NumberFormat("en", { notation: "compact", maximumFractionDigits: 1 }).format(n);
}

function shortTime(iso: string): string {
  if (!iso) return "—";
  const d = new Date(iso);
  if (Number.isNaN(d.getTime())) return iso;
  return d.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
}

export default function OverviewPage() {
  const [config, setConfig] = useState<UiConfig | null>(null);
  const [stack, setStack] = useState<StackStatus | null>(null);
  const [overview, setOverview] = useState<OverviewDashboard | null>(null);
  const [casesCount, setCasesCount] = useState<number | null>(null);
  const [alertsCount, setAlertsCount] = useState<number | null>(null);
  const [stats, setStats] = useState<{ rules_count: number; pending_alerts: number } | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    Promise.all([uiConfig(), stackStatus(), getOverviewDashboard(), listCases({ limit: "1" }), getAlerts(), getCorrelatorStats()])
      .then(([cfg, stackData, overviewData, cases, alerts, correlator]) => {
        if (!active) return;
        setConfig(cfg);
        setStack(stackData);
        setOverview(overviewData);
        setCasesCount(cases.total);
        setAlertsCount(alerts.length);
        setStats({ rules_count: correlator.rules_count, pending_alerts: correlator.pending_alerts });
      })
      .catch((e) => {
        if (!active) return;
        setErr(String(e));
      });
    return () => {
      active = false;
    };
  }, []);

  const maxEvents = useMemo(
    () => Math.max(...(overview?.events_per_minute.map((point) => point.events) ?? [0]), 1),
    [overview]
  );

  const trendBars = useMemo(
    () =>
      (overview?.events_per_minute ?? []).map((point) => ({
        ...point,
        ratio: Math.max(8, Math.round((point.events / maxEvents) * 100)),
      })),
    [overview, maxEvents]
  );

  return (
    <div className="page-grid overview-dashboard">
      {err && <p className="error">{err}</p>}
      <section className="card hero-card">
        <h2>SOC overview, но уже наш</h2>
        <p className="meta">
          Нативный dashboard поверх ClickHouse и сервисов стека: без Grafana-iframe, но с теми же основными сигналами
          для ежедневной работы.
        </p>
        <div className="kpi-grid">
          <div className="kpi-card">
            <span>Total events ({overview?.window_hours ?? 24}h)</span>
            <strong>{compact(overview?.kpis.total_events_24h)}</strong>
          </div>
          <div className="kpi-card">
            <span>Critical events</span>
            <strong>{compact(overview?.kpis.critical_events_24h)}</strong>
          </div>
          <div className="kpi-card">
            <span>Error + critical share</span>
            <strong>{overview ? `${overview.kpis.error_pct_24h.toFixed(2)}%` : "—"}</strong>
          </div>
          <div className="kpi-card">
            <span>Open cases</span>
            <strong>{compact(casesCount)}</strong>
          </div>
          <div className="kpi-card">
            <span>Active alerts</span>
            <strong>{compact(alertsCount)}</strong>
          </div>
          <div className="kpi-card">
            <span>Detection rules</span>
            <strong>{compact(stats?.rules_count)}</strong>
          </div>
          <div className="kpi-card">
            <span>Pending forwards</span>
            <strong>{compact(stats?.pending_alerts)}</strong>
          </div>
        </div>
        <div className="btn-row">
          <Link className="tool-btn secondary" to="/infrastructure">
            Open infrastructure
          </Link>
          <Link className="tool-btn" to="/alerts">
            Open alert inbox
          </Link>
          <Link className="tool-btn secondary" to="/dashboards">
            Open dashboards
          </Link>
          <Link className="tool-btn" to="/events">
            Search events
          </Link>
          <Link className="tool-btn secondary" to="/cases">
            Review cases
          </Link>
        </div>
      </section>

      <section className="overview-grid">
        <article className="card">
          <h2>Events per minute</h2>
          {!trendBars.length ? (
            <p className="meta">Пока нет данных в агрегате `events_per_minute_agg`.</p>
          ) : (
            <div className="mini-bars">
              {trendBars.map((point) => (
                <div key={point.minute} className="mini-bar-col" title={`${point.minute}: ${point.events}`}>
                  <div className="mini-bar-track">
                    <div className="mini-bar-fill" style={{ height: `${point.ratio}%` }} />
                  </div>
                  <span>{shortTime(point.minute)}</span>
                </div>
              ))}
            </div>
          )}
        </article>

        <article className="card">
          <h2>Events by severity</h2>
          {!overview?.severity_breakdown.length ? (
            <p className="meta">Нет severity breakdown за последние 24 часа.</p>
          ) : (
            <div className="metric-list">
              {overview.severity_breakdown.map((row) => (
                <div key={row.severity} className="metric-row">
                  <span className={`badge sev-${row.severity.toLowerCase()}`}>{row.severity}</span>
                  <strong>{compact(row.events)}</strong>
                </div>
              ))}
            </div>
          )}
        </article>
      </section>

      <section className="overview-grid">
        <article className="card">
          <h2>Top source IPs</h2>
          {!overview?.top_source_ips.length ? (
            <p className="meta">Нет source IP c трафиком за последние 24 часа.</p>
          ) : (
            <table>
              <thead>
                <tr>
                  <th>IP</th>
                  <th>Events</th>
                  <th>Threats</th>
                </tr>
              </thead>
              <tbody>
                {overview.top_source_ips.map((row) => (
                  <tr key={row.source_ip}>
                    <td>{row.source_ip}</td>
                    <td>{compact(row.events)}</td>
                    <td>{compact(row.threats)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </article>

        <article className="card">
          <h2>Recent security events</h2>
          {!overview?.recent_security_events.length ? (
            <p className="meta">Нет recent security events в `siem.events`.</p>
          ) : (
            <div className="recent-event-stack">
              {overview.recent_security_events.slice(0, 8).map((row) => (
                <button
                  type="button"
                  key={row.event_id}
                  className="recent-event-card"
                  onClick={() => {
                    window.location.href = `/events?q=${encodeURIComponent(row.source_ip || row.host || row.source_type)}`;
                  }}
                >
                  <header>
                    <span>{shortTime(row.timestamp)}</span>
                    <span className={`badge sev-${row.severity.toLowerCase()}`}>{row.severity}</span>
                  </header>
                  <strong>{row.source_type}</strong>
                  <p>{row.message}</p>
                  <small>{row.source_ip || row.host || row.event_id}</small>
                </button>
              ))}
            </div>
          )}
        </article>
      </section>

      <section className="overview-grid">
        <article className="card">
          <h2>Top sources</h2>
          {!overview?.source_breakdown.length ? (
            <p className="meta">Нет source breakdown по агрегату.</p>
          ) : (
            <div className="metric-list">
              {overview.source_breakdown.map((row) => (
                <div key={row.source_type} className="metric-row">
                  <span>{row.source_type}</span>
                  <strong>{compact(row.events)}</strong>
                </div>
              ))}
            </div>
          )}
        </article>

        <article className="card">
          <h2>Stack health</h2>
          {!stack ? (
            <p className="meta">Загрузка…</p>
          ) : (
            <table>
              <thead>
                <tr>
                  <th>Component</th>
                  <th>Status</th>
                  <th>Latency</th>
                </tr>
              </thead>
              <tbody>
                {Object.entries(stack.components).map(([name, value]) => (
                  <tr key={name}>
                    <td>{name}</td>
                    <td>
                      <span className={`badge ${value.ok ? "sev-low" : "sev-critical"}`}>{value.ok ? "ok" : "down"}</span>
                    </td>
                    <td>{value.latency_ms ?? "—"} ms</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
        </article>
      </section>

      <section className="card">
        <h2>Deep-dive and fallbacks</h2>
        <div className="home-grid">
          <Link className="home-card" to="/infrastructure">
            <h2>Native infrastructure</h2>
            <p>Наш собственный Prometheus-based экран: CPU, RAM, disk, network, containers и component status.</p>
          </Link>
          <Link className="home-card" to="/dashboards">
            <h2>Embedded dashboards</h2>
            <p>Grafana остаётся внутри suite как fallback, пока мы переносим dashboard-ы в свой UI.</p>
          </Link>
          <a className="home-card" href={config?.links.grafana || "#"} target="_blank" rel="noreferrer">
            <h2>Grafana</h2>
            <p>Explore, сложные инженерные панели и deep-dive по ClickHouse/Loki.</p>
          </a>
          <a
            className="home-card"
            href={config?.links.siem_overview_dashboard || config?.links.grafana || "#"}
            target="_blank"
            rel="noreferrer"
          >
            <h2>Legacy overview dashboard</h2>
            <p>Старый Grafana-обзор как резерв, пока мы наращиваем свой нативный экран.</p>
          </a>
          <a className="home-card" href={config?.links.prometheus || "#"} target="_blank" rel="noreferrer">
            <h2>Prometheus</h2>
            <p>Проверка PromQL, target health и инженерные deep-dive метрики.</p>
          </a>
        </div>
      </section>
    </div>
  );
}
