import { useCallback, useEffect, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { getInfrastructureDashboard, uiConfig, type InfrastructureDashboard, type UiConfig } from "../api";
import DashboardToolbar from "../components/DashboardToolbar";
import { NativeBarChart, NativeGaugeChart, NativeLineChart } from "../components/NativeCharts";
import { formatBytes, formatCompact, formatPercent, formatRate, formatUptime } from "../dashboard-utils";

export default function InfrastructurePage() {
  const [config, setConfig] = useState<UiConfig | null>(null);
  const [data, setData] = useState<InfrastructureDashboard | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [hours, setHours] = useState(6);
  const [autoRefreshSec, setAutoRefreshSec] = useState(0);
  const [loading, setLoading] = useState(false);
  const mounted = useRef(true);
  const requestSeq = useRef(0);

  const load = useCallback(() => {
    if (!mounted.current) return;
    const seq = ++requestSeq.current;
    setLoading(true);
    Promise.all([uiConfig(), getInfrastructureDashboard(hours)])
      .then(([cfg, dashboard]) => {
        if (!mounted.current || seq !== requestSeq.current) return;
        setConfig(cfg);
        setData(dashboard);
        setErr(null);
      })
      .catch((error) => {
        if (!mounted.current || seq !== requestSeq.current) return;
        setErr(String(error));
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
    <div className="page-grid infrastructure-page">
      {err && <p className="error">{err}</p>}

      <DashboardToolbar
        title="Infrastructure, но уже в нашем UI"
        subtitle="Нативный экран поверх Prometheus: host CPU/RAM/disk/network, контейнеры и состояние компонентов, с диапазоном и автообновлением."
        hours={hours}
        autoRefreshSec={autoRefreshSec}
        loading={loading}
        onHoursChange={setHours}
        onAutoRefreshChange={setAutoRefreshSec}
        onRefresh={load}
      />

      <section className="card">
        <p className="meta">
          На Windows + Docker Desktop host-метрики всё ещё показывают Linux VM Docker, а не голый Windows host.
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

      <section className="dashboard-gauge-grid">
        <NativeGaugeChart
          title="CPU usage"
          value={data?.host.cpu_usage_pct}
          detail="Host pressure"
          formatter={(value) => formatPercent(value)}
        />
        <NativeGaugeChart
          title="Memory usage"
          value={data?.host.memory_usage_pct}
          detail="Working set pressure"
          formatter={(value) => formatPercent(value)}
        />
        <NativeGaugeChart
          title="Disk usage"
          value={data?.host.disk_usage_pct}
          detail="Storage saturation"
          formatter={(value) => formatPercent(value)}
        />
        <NativeGaugeChart
          title="Component health"
          value={data ? (data.host.healthy_components / Math.max(data.host.total_components, 1)) * 100 : null}
          detail="Reachable components"
          formatter={(value) => formatPercent(value)}
        />
      </section>

      <section className="infra-grid">
        <article className="card">
          <h2>Host CPU trend</h2>
          <NativeLineChart
            title="Host CPU trend"
            color="#7be37c"
            points={(data?.cpu_series ?? []).map((point) => ({ x: String(point.ts), y: point.value }))}
            filled
            fillOpacity={0.2}
          />
          <p className="meta stat-subtle">
            Последние {data?.window_hours ?? 6} часов, шаг {Math.round((data?.step_sec ?? 300) / 60)} минут.
          </p>
        </article>

        <article className="card">
          <h2>Network I/O trend</h2>
          <div className="sparkline-stack">
            <NativeLineChart
              title="Network RX trend"
              color="#4d9bff"
              points={(data?.network_rx_series ?? []).map((point) => ({ x: String(point.ts), y: point.value }))}
            />
            <NativeLineChart
              title="Network TX trend"
              color="#f0c15d"
              points={(data?.network_tx_series ?? []).map((point) => ({ x: String(point.ts), y: point.value }))}
            />
          </div>
          <p className="meta stat-subtle">Blue = RX, yellow = TX.</p>
        </article>
      </section>

      <section className="infra-grid">
        <article className="card">
          <h2>Top containers by CPU</h2>
          {!data?.top_cpu_containers.length ? (
            <p className="meta">cAdvisor не вернул per-container CPU breakdown.</p>
          ) : (
            <NativeBarChart
              title="Top containers by CPU"
              rows={data.top_cpu_containers.map((row) => ({
                label: row.name,
                value: Number(row.value.toFixed(2)),
              }))}
              color="linear-gradient(90deg, #4d9bff 0%, #7be37c 100%)"
              valueFormatter={(value) => `${value.toFixed(1)}%`}
            />
          )}
          <p className="meta stat-subtle">Total container CPU: {formatPercent(data?.host.total_container_cpu_pct)}</p>
        </article>

        <article className="card">
          <h2>Top containers by memory</h2>
          {!data?.top_memory_containers.length ? (
            <p className="meta">cAdvisor не вернул per-container memory breakdown.</p>
          ) : (
            <NativeBarChart
              title="Top containers by memory"
              rows={data.top_memory_containers.map((row) => ({
                label: row.name,
                value: Number(row.value.toFixed(0)),
              }))}
              color="linear-gradient(90deg, #8f6dff 0%, #4d9bff 100%)"
              valueFormatter={(value) => formatBytes(value)}
            />
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
