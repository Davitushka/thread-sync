import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { getInfrastructureDashboard, uiConfig, type InfrastructureDashboard, type UiConfig } from "../api";
import DashboardToolbar from "../components/DashboardToolbar";
import { formatBytes, formatCompact, formatPercent, formatRate, formatUptime } from "../dashboard-utils";
import {
  ObservabilityBarPanel,
  ObservabilityGaugePanel,
  ObservabilityLinePanel,
  ObservabilityPanel,
} from "../components/echarts/ObservabilityCharts";

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

  const timelineLabels = useMemo(
    () =>
      (data?.cpu_series ?? []).map((point) =>
        new Date(point.ts * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
      ),
    [data?.cpu_series]
  );

  const networkLabels = useMemo(
    () =>
      (data?.network_rx_series ?? []).map((point) =>
        new Date(point.ts * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
      ),
    [data?.network_rx_series]
  );

  return (
    <div className="page-grid infrastructure-page">
      {err && <p className="error">{err}</p>}

      <DashboardToolbar
        title="Infrastructure command surface"
        subtitle="Native observability dashboard over Prometheus for host pressure, container behavior and platform reachability, rebuilt as a denser Grafana-like pilot."
        hours={hours}
        autoRefreshSec={autoRefreshSec}
        loading={loading}
        onHoursChange={setHours}
        onAutoRefreshChange={setAutoRefreshSec}
        onRefresh={load}
      />

      <section className="card">
        <div className="workspace-pane-header">
          <div className="workspace-pane-copy">
            <span className="workspace-pane-kicker">Pilot screen</span>
            <h2>Infrastructure rebuilt on ECharts</h2>
            <p className="workspace-pane-subtitle">
              This pilot keeps our own shell and data APIs, but replaces the hand-drawn chart language with denser observability panels closer to a real monitoring product.
            </p>
          </div>
        </div>
        <p className="meta">On Windows + Docker Desktop the host metrics still describe the Linux VM behind Docker rather than the bare Windows host.</p>
        <div className="btn-row">
          <Link className="tool-btn" to="/dashboards">
            Open dashboards hub
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
        <ObservabilityGaugePanel
          title="CPU usage"
          value={data?.host.cpu_usage_pct}
          subtitle="Host saturation"
          formatter={(value) => formatPercent(value)}
          footer={<p className="meta stat-subtle">Keep this below sustained warning ranges during ingestion spikes.</p>}
        />
        <ObservabilityGaugePanel
          title="Memory usage"
          value={data?.host.memory_usage_pct}
          subtitle="Working set pressure"
          formatter={(value) => formatPercent(value)}
          footer={<p className="meta stat-subtle">Memory pressure usually appears before parser or ClickHouse instability.</p>}
        />
        <ObservabilityGaugePanel
          title="Disk usage"
          value={data?.host.disk_usage_pct}
          subtitle="Storage saturation"
          formatter={(value) => formatPercent(value)}
          footer={<p className="meta stat-subtle">Storage pressure directly impacts retention, lag and query responsiveness.</p>}
        />
        <ObservabilityGaugePanel
          title="Component health"
          value={data ? (data.host.healthy_components / Math.max(data.host.total_components, 1)) * 100 : null}
          subtitle="Reachable components"
          formatter={(value) => formatPercent(value)}
          footer={<p className="meta stat-subtle">A fast read on how much of the platform is actually reachable right now.</p>}
        />
      </section>

      <section className="observability-grid observability-grid-primary">
        <ObservabilityLinePanel
          title="Host CPU timeline"
          subtitle={`Last ${data?.window_hours ?? 6}h, step ${Math.round((data?.step_sec ?? 300) / 60)} minutes`}
          categories={timelineLabels}
          series={[
            {
              name: "cpu %",
              color: "#7be37c",
              data: (data?.cpu_series ?? []).map((point) => point.value),
              areaOpacity: 0.18,
            },
          ]}
          axisFormatter={(value) => `${Math.round(value)}%`}
          valueFormatter={(value) => `${value.toFixed(1)}%`}
          className="observability-panel-wide"
          footer={<p className="meta stat-subtle">This should stay visually calm; frequent spikes usually reflect ingestion bursts or resource contention.</p>}
        />

        <ObservabilityPanel
          title="Platform snapshot"
          subtitle="Dense operational summary"
          className="observability-panel-compact"
          footer={<p className="meta stat-subtle">Use this side panel as the fast comparison layer before jumping into the deeper trend panels.</p>}
        >
          <div className="observability-stat-stack">
            <div className="observability-stat-card">
              <span>Network RX</span>
              <strong>{formatRate(data?.host.network_rx_bps)}</strong>
            </div>
            <div className="observability-stat-card">
              <span>Network TX</span>
              <strong>{formatRate(data?.host.network_tx_bps)}</strong>
            </div>
            <div className="observability-stat-card">
              <span>Uptime</span>
              <strong>{formatUptime(data?.host.uptime_sec)}</strong>
            </div>
            <div className="observability-stat-card">
              <span>Containers</span>
              <strong>{formatCompact(data?.host.container_count)}</strong>
            </div>
            <div className="observability-stat-card">
              <span>Total container CPU</span>
              <strong>{formatPercent(data?.host.total_container_cpu_pct)}</strong>
            </div>
            <div className="observability-stat-card">
              <span>Total container memory</span>
              <strong>{formatBytes(data?.host.total_container_memory_bytes)}</strong>
            </div>
          </div>
        </ObservabilityPanel>
      </section>

      <section className="observability-grid">
        <ObservabilityLinePanel
          title="Network I/O"
          subtitle="Receive versus transmit"
          categories={networkLabels}
          series={[
            {
              name: "rx",
              color: "#4d9bff",
              data: (data?.network_rx_series ?? []).map((point) => point.value),
              areaOpacity: 0.14,
            },
            {
              name: "tx",
              color: "#f0c15d",
              data: (data?.network_tx_series ?? []).map((point) => point.value),
            },
          ]}
          axisFormatter={(value) => formatCompact(value)}
          valueFormatter={(value) => formatRate(value)}
          footer={<p className="meta stat-subtle">Blue is receive, amber is transmit. Watch for asymmetry during collector or parser pressure.</p>}
        />

        <ObservabilityBarPanel
          title="Top containers by CPU"
          subtitle="Most expensive workloads"
          rows={(data?.top_cpu_containers ?? []).map((row) => ({
            label: row.name,
            value: Number(row.value.toFixed(2)),
            color: "#7be37c",
          }))}
          axisFormatter={(value) => `${value.toFixed(0)}%`}
          valueFormatter={(value) => `${value.toFixed(1)}%`}
          footer={<p className="meta stat-subtle">Use this to spot which container is actually burning the platform budget.</p>}
        />
      </section>

      <section className="observability-grid">
        <ObservabilityBarPanel
          title="Top containers by memory"
          subtitle="Largest working sets"
          rows={(data?.top_memory_containers ?? []).map((row) => ({
            label: row.name,
            value: Number(row.value.toFixed(0)),
            color: "#8f6dff",
          }))}
          axisFormatter={(value) => formatCompact(value)}
          valueFormatter={(value) => formatBytes(value)}
          footer={<p className="meta stat-subtle">This is often the earliest signal that memory pressure is concentrated in one service.</p>}
        />

        <ObservabilityPanel
          title="Component status matrix"
          subtitle="Reachability by service"
          footer={
            !data?.component_status.length ? (
              <p className="meta stat-subtle">
                Prometheus <code>up{"{job=...}"}</code> did not return component health for this window.
              </p>
            ) : (
              <p className="meta stat-subtle">Healthy services stay green. Red entries usually explain why dashboards or pipelines feel stale.</p>
            )
          }
        >
          <div className="infra-health-grid">
            {data?.component_status.map((item) => (
              <div key={item.job} className={`health-card ${item.up ? "health-card-up" : "health-card-down"}`}>
                <div className="health-card-copy">
                  <strong>{item.job}</strong>
                  <small>{item.up ? "Reachable via Prometheus" : "Missing or degraded target"}</small>
                </div>
                <span className={`badge ${item.up ? "sev-low" : "sev-critical"}`}>{item.up ? "up" : "down"}</span>
              </div>
            )) ?? null}
          </div>
        </ObservabilityPanel>
      </section>
    </div>
  );
}
