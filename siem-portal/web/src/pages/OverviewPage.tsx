import { useCallback, useEffect, useRef, useState } from "react";
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
import DashboardToolbar from "../components/DashboardToolbar";
import { NativeBarChart, NativeLineChart, NativeMultiLineChart } from "../components/NativeCharts";
import { formatCompact, shortDateTime } from "../dashboard-utils";

export default function OverviewPage() {
  const [config, setConfig] = useState<UiConfig | null>(null);
  const [stack, setStack] = useState<StackStatus | null>(null);
  const [overview, setOverview] = useState<OverviewDashboard | null>(null);
  const [casesCount, setCasesCount] = useState<number | null>(null);
  const [alertsCount, setAlertsCount] = useState<number | null>(null);
  const [stats, setStats] = useState<{ rules_count: number; pending_alerts: number } | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [hours, setHours] = useState(24);
  const [autoRefreshSec, setAutoRefreshSec] = useState(0);
  const [loading, setLoading] = useState(false);
  const mounted = useRef(true);
  const requestSeq = useRef(0);

  const load = useCallback(() => {
    if (!mounted.current) return;
    const seq = ++requestSeq.current;
    setLoading(true);
    Promise.all([uiConfig(), stackStatus(), getOverviewDashboard(hours), listCases({ limit: "1" }), getAlerts(), getCorrelatorStats()])
      .then(([cfg, stackData, overviewData, cases, alerts, correlator]) => {
        if (!mounted.current || seq !== requestSeq.current) return;
        setConfig(cfg);
        setStack(stackData);
        setOverview(overviewData);
        setCasesCount(cases.total);
        setAlertsCount(alerts.length);
        setStats({ rules_count: correlator.rules_count, pending_alerts: correlator.pending_alerts });
        setErr(null);
      })
      .catch((e) => {
        if (!mounted.current || seq !== requestSeq.current) return;
        setErr(String(e));
      })
      .finally(() => {
        if (!mounted.current || seq !== requestSeq.current) return;
        setLoading(false);
      });
  }, [hours]);

  useEffect(() => {
    return () => {
      mounted.current = false;
    };
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  useEffect(() => {
    if (!autoRefreshSec) return;
    const id = window.setInterval(() => load(), autoRefreshSec * 1000);
    return () => window.clearInterval(id);
  }, [autoRefreshSec, load]);

  return (
    <div className="page-grid overview-dashboard">
      {err && <p className="error">{err}</p>}
      <DashboardToolbar
        title="SOC overview, но уже наш"
        subtitle="Нативный dashboard поверх ClickHouse и сервисов стека: диапазон, автообновление и свои графики без Grafana-iframe."
        hours={hours}
        autoRefreshSec={autoRefreshSec}
        loading={loading}
        onHoursChange={setHours}
        onAutoRefreshChange={setAutoRefreshSec}
        onRefresh={load}
      />

      <section className="card">
        <div className="kpi-grid">
          <div className="kpi-card">
            <span>Total events ({overview?.window_hours ?? hours}h)</span>
            <strong>{formatCompact(overview?.kpis.total_events_24h)}</strong>
          </div>
          <div className="kpi-card">
            <span>Critical events</span>
            <strong>{formatCompact(overview?.kpis.critical_events_24h)}</strong>
          </div>
          <div className="kpi-card">
            <span>Error + critical share</span>
            <strong>{overview ? `${overview.kpis.error_pct_24h.toFixed(2)}%` : "—"}</strong>
          </div>
          <div className="kpi-card">
            <span>Open cases</span>
            <strong>{formatCompact(casesCount)}</strong>
          </div>
          <div className="kpi-card">
            <span>Active alerts</span>
            <strong>{formatCompact(alertsCount)}</strong>
          </div>
          <div className="kpi-card">
            <span>Detection rules</span>
            <strong>{formatCompact(stats?.rules_count)}</strong>
          </div>
          <div className="kpi-card">
            <span>Pending forwards</span>
            <strong>{formatCompact(stats?.pending_alerts)}</strong>
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
          <h2>Events timeline</h2>
          {!overview?.events_per_minute.length ? (
            <p className="meta">Пока нет данных в агрегате `events_per_minute_agg`.</p>
          ) : (
            <>
              <NativeLineChart
                title="Events timeline"
                color="#7be37c"
                points={overview.events_per_minute.map((point) => ({ x: point.minute, y: point.events }))}
              />
              <p className="meta stat-subtle">
                Bucket = {overview.bucket_minutes} min, range = {overview.window_hours}h.
              </p>
            </>
          )}
        </article>

        <article className="card">
          <h2>Events by severity</h2>
          {!overview?.severity_breakdown.length ? (
            <p className="meta">Нет severity breakdown за последние {overview?.window_hours ?? hours} часов.</p>
          ) : (
            <NativeBarChart
              title="Events by severity"
              rows={overview.severity_breakdown.map((row) => ({
                label: row.severity,
                value: row.events,
                tone:
                  row.severity === "critical"
                    ? "#f85149"
                    : row.severity === "error"
                      ? "#f0883e"
                      : row.severity === "warning"
                        ? "#d29922"
                        : "#3fb950",
              }))}
              valueFormatter={(value) => formatCompact(value)}
            />
          )}
        </article>
      </section>

      <section className="card">
        <h2>Severity timeline</h2>
        {!overview?.severity_timeline.length ? (
          <p className="meta">Нет severity timeline за выбранный диапазон.</p>
        ) : (
          <NativeMultiLineChart
            title="Severity timeline"
            points={overview.severity_timeline.map((row) => ({
              x: row.bucket,
              critical: row.critical,
              error: row.error,
              warning: row.warning,
            }))}
            series={[
              { key: "critical", label: "critical", color: "#f85149" },
              { key: "error", label: "error", color: "#f0883e" },
              { key: "warning", label: "warning", color: "#d29922" },
            ]}
          />
        )}
      </section>

      <section className="overview-grid">
        <article className="card">
          <h2>Top source IPs</h2>
          {!overview?.top_source_ips.length ? (
            <p className="meta">Нет source IP c трафиком за последние {overview?.window_hours ?? hours} часов.</p>
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
                    <td>{formatCompact(row.events)}</td>
                    <td>{formatCompact(row.threats)}</td>
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
                    <span>{shortDateTime(row.timestamp)}</span>
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
            <NativeBarChart
              title="Top sources"
              rows={overview.source_breakdown.map((row) => ({
                label: row.source_type,
                value: row.events,
              }))}
              valueFormatter={(value) => formatCompact(value)}
            />
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
