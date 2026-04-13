import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { getInfrastructureDashboard, uiConfig, type InfrastructureDashboard, type UiConfig } from "../api";

function formatCompact(value: number | null | undefined): string {
  if (value == null || Number.isNaN(value)) return "—";
  return new Intl.NumberFormat("en", { notation: "compact", maximumFractionDigits: 1 }).format(value);
}

function formatPercent(value: number | null | undefined): string {
  if (value == null || Number.isNaN(value)) return "—";
  return `${value.toFixed(1)}%`;
}

function formatBytes(value: number | null | undefined): string {
  if (value == null || Number.isNaN(value)) return "—";
  const units = ["B", "KB", "MB", "GB", "TB"];
  let size = Math.max(value, 0);
  let idx = 0;
  while (size >= 1024 && idx < units.length - 1) {
    size /= 1024;
    idx += 1;
  }
  return `${size.toFixed(size >= 10 || idx === 0 ? 0 : 1)} ${units[idx]}`;
}

function formatRate(value: number | null | undefined): string {
  if (value == null || Number.isNaN(value)) return "—";
  return `${formatBytes(value)}/s`;
}

function formatUptime(seconds: number | null | undefined): string {
  if (seconds == null || Number.isNaN(seconds)) return "—";
  const hours = seconds / 3600;
  if (hours >= 48) return `${(hours / 24).toFixed(1)} days`;
  return `${hours.toFixed(1)} hours`;
}

type Point = { ts: number; value: number };

function sparkPath(points: Point[], width: number, height: number): string {
  if (!points.length) return "";
  const min = Math.min(...points.map((point) => point.value), 0);
  const max = Math.max(...points.map((point) => point.value), 1);
  const span = Math.max(max - min, 1);
  return points
    .map((point, idx) => {
      const x = (idx / Math.max(points.length - 1, 1)) * width;
      const y = height - ((point.value - min) / span) * height;
      return `${x.toFixed(2)},${y.toFixed(2)}`;
    })
    .join(" ");
}

function Sparkline({
  points,
  color,
  title,
}: {
  points: Point[];
  color: string;
  title: string;
}) {
  const path = useMemo(() => sparkPath(points, 100, 42), [points]);
  return (
    <div className="sparkline-shell">
      <svg className="sparkline-svg" viewBox="0 0 100 42" role="img" aria-label={title}>
        <polyline className="sparkline-gridline" points="0,41 100,41" />
        {path ? <polyline points={path} fill="none" stroke={color} strokeWidth="2.5" strokeLinejoin="round" /> : null}
      </svg>
    </div>
  );
}

export default function InfrastructurePage() {
  const [config, setConfig] = useState<UiConfig | null>(null);
  const [data, setData] = useState<InfrastructureDashboard | null>(null);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    let active = true;
    Promise.all([uiConfig(), getInfrastructureDashboard()])
      .then(([cfg, dashboard]) => {
        if (!active) return;
        setConfig(cfg);
        setData(dashboard);
      })
      .catch((error) => {
        if (!active) return;
        setErr(String(error));
      });
    return () => {
      active = false;
    };
  }, []);

  return (
    <div className="page-grid infrastructure-page">
      {err && <p className="error">{err}</p>}

      <section className="card hero-card">
        <h2>Infrastructure, но уже в нашем UI</h2>
        <p className="meta">
          Нативный экран поверх Prometheus: host CPU/RAM/disk/network, контейнеры и состояние компонентов без Grafana
          dashboard. На Windows + Docker Desktop host-метрики всё ещё показывают Linux VM Docker, а не голый Windows
          host.
        </p>
        <div className="btn-row">
          <Link className="tool-btn" to="/dashboards">
            Open embedded dashboards
          </Link>
          <a className="tool-btn secondary" href={config?.links.prometheus || "#"} target="_blank" rel="noreferrer">
            Open Prometheus
          </a>
          <a className="tool-btn secondary" href={config?.links.grafana || "#"} target="_blank" rel="noreferrer">
            Open Grafana
          </a>
        </div>
      </section>

      <section className="kpi-grid">
        <div className="kpi-card">
          <span>Host CPU</span>
          <strong>{formatPercent(data?.host.cpu_usage_pct)}</strong>
        </div>
        <div className="kpi-card">
          <span>Host Memory</span>
          <strong>{formatPercent(data?.host.memory_usage_pct)}</strong>
        </div>
        <div className="kpi-card">
          <span>Host Disk</span>
          <strong>{formatPercent(data?.host.disk_usage_pct)}</strong>
        </div>
        <div className="kpi-card">
          <span>Network RX</span>
          <strong>{formatRate(data?.host.network_rx_bps)}</strong>
        </div>
        <div className="kpi-card">
          <span>Network TX</span>
          <strong>{formatRate(data?.host.network_tx_bps)}</strong>
        </div>
        <div className="kpi-card">
          <span>Uptime</span>
          <strong>{formatUptime(data?.host.uptime_sec)}</strong>
        </div>
        <div className="kpi-card">
          <span>Running containers</span>
          <strong>{formatCompact(data?.host.container_count)}</strong>
        </div>
        <div className="kpi-card">
          <span>Components healthy</span>
          <strong>
            {data ? `${data.host.healthy_components}/${data.host.total_components}` : "—"}
          </strong>
        </div>
      </section>

      <section className="infra-grid">
        <article className="card">
          <h2>Host CPU trend</h2>
          <Sparkline points={data?.cpu_series ?? []} color="#7be37c" title="Host CPU trend" />
          <p className="meta stat-subtle">
            Последние {data?.window_hours ?? 6} часов, шаг {Math.round((data?.step_sec ?? 300) / 60)} минут.
          </p>
        </article>

        <article className="card">
          <h2>Network I/O trend</h2>
          <div className="sparkline-stack">
            <div>
              <Sparkline points={data?.network_rx_series ?? []} color="#4d9bff" title="Network RX trend" />
              <p className="meta stat-subtle">RX</p>
            </div>
            <div>
              <Sparkline points={data?.network_tx_series ?? []} color="#f0c15d" title="Network TX trend" />
              <p className="meta stat-subtle">TX</p>
            </div>
          </div>
        </article>
      </section>

      <section className="infra-grid">
        <article className="card">
          <h2>Top containers by CPU</h2>
          {!data?.top_cpu_containers.length ? (
            <p className="meta">cAdvisor не вернул per-container CPU breakdown.</p>
          ) : (
            <table>
              <thead>
                <tr>
                  <th>Container</th>
                  <th>CPU %</th>
                </tr>
              </thead>
              <tbody>
                {data.top_cpu_containers.map((row) => (
                  <tr key={row.name}>
                    <td>{row.name}</td>
                    <td>{formatPercent(row.value)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          <p className="meta stat-subtle">Total container CPU: {formatPercent(data?.host.total_container_cpu_pct)}</p>
        </article>

        <article className="card">
          <h2>Top containers by memory</h2>
          {!data?.top_memory_containers.length ? (
            <p className="meta">cAdvisor не вернул per-container memory breakdown.</p>
          ) : (
            <table>
              <thead>
                <tr>
                  <th>Container</th>
                  <th>Memory</th>
                </tr>
              </thead>
              <tbody>
                {data.top_memory_containers.map((row) => (
                  <tr key={row.name}>
                    <td>{row.name}</td>
                    <td>{formatBytes(row.value)}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}
          <p className="meta stat-subtle">Total container memory: {formatBytes(data?.host.total_container_memory_bytes)}</p>
        </article>
      </section>

      <section className="card">
        <h2>Component status</h2>
        {!data?.component_status.length ? (
          <p className="meta">
            Prometheus <code>up{"{job=...}"}</code> не вернул компонентный статус.
          </p>
        ) : (
          <div className="infra-health-grid">
            {data.component_status.map((item) => (
              <div key={item.job} className="health-card">
                <strong>{item.job}</strong>
                <span className={`badge ${item.up ? "sev-low" : "sev-critical"}`}>{item.up ? "up" : "down"}</span>
              </div>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
