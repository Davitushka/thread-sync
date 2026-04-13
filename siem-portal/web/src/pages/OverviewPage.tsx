import { useEffect, useState } from "react";
import { Link } from "react-router-dom";
import { getAlerts, getCorrelatorStats, listCases, stackStatus, uiConfig, type StackStatus, type UiConfig } from "../api";

export default function OverviewPage() {
  const [config, setConfig] = useState<UiConfig | null>(null);
  const [stack, setStack] = useState<StackStatus | null>(null);
  const [casesCount, setCasesCount] = useState<number | null>(null);
  const [alertsCount, setAlertsCount] = useState<number | null>(null);
  const [stats, setStats] = useState<{ rules_count: number; pending_alerts: number } | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    Promise.all([uiConfig(), stackStatus(), listCases({ limit: "1" }), getAlerts(), getCorrelatorStats()])
      .then(([cfg, stackData, cases, alerts, correlator]) => {
        if (!active) return;
        setConfig(cfg);
        setStack(stackData);
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

  return (
    <div className="page-grid">
      {err && <p className="error">{err}</p>}
      <section className="card hero-card">
        <h2>Одна рабочая поверхность</h2>
        <p className="meta">
          Портал теперь выступает как единый BFF: cases, detections, alerts и event search идут через один origin.
        </p>
        <div className="kpi-grid">
          <div className="kpi-card">
            <span>Open cases</span>
            <strong>{casesCount ?? "—"}</strong>
          </div>
          <div className="kpi-card">
            <span>Active alerts</span>
            <strong>{alertsCount ?? "—"}</strong>
          </div>
          <div className="kpi-card">
            <span>Detection rules</span>
            <strong>{stats?.rules_count ?? "—"}</strong>
          </div>
          <div className="kpi-card">
            <span>Pending forwards</span>
            <strong>{stats?.pending_alerts ?? "—"}</strong>
          </div>
        </div>
        <div className="btn-row">
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

      <section className="card">
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
      </section>

      <section className="card">
        <h2>External tools</h2>
        <div className="home-grid">
          <Link className="home-card" to="/dashboards">
            <h2>Embedded dashboards</h2>
            <p>Overview, infrastructure, validation, ClickHouse и другие Grafana dashboards внутри suite.</p>
          </Link>
          <a className="home-card" href={config?.links.grafana || "#"} target="_blank" rel="noreferrer">
            <h2>Grafana</h2>
            <p>Dashboards, Explore и deep-dive по ClickHouse/Loki.</p>
          </a>
          <a
            className="home-card"
            href={config?.links.siem_overview_dashboard || config?.links.grafana || "#"}
            target="_blank"
            rel="noreferrer"
          >
            <h2>Overview dashboard</h2>
            <p>Быстрый переход в основной обзор SIEM-графиков.</p>
          </a>
          <a className="home-card" href={config?.links.prometheus || "#"} target="_blank" rel="noreferrer">
            <h2>Prometheus</h2>
            <p>Проверка PromQL и target health.</p>
          </a>
        </div>
      </section>
    </div>
  );
}
