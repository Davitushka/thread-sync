import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { getDataQualityDashboard, uiConfig, type DataQualityDashboard, type UiConfig } from "../api";
import DashboardToolbar from "../components/DashboardToolbar";
import { formatCompact, formatPercent } from "../dashboard-utils";
import {
  ObservabilityBarPanel,
  ObservabilityGaugePanel,
  ObservabilityLinePanel,
} from "../components/echarts/ObservabilityCharts";

export default function DataQualityPage() {
  const [config, setConfig] = useState<UiConfig | null>(null);
  const [data, setData] = useState<DataQualityDashboard | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [hours, setHours] = useState(24);
  const [autoRefreshSec, setAutoRefreshSec] = useState(60);
  const [loading, setLoading] = useState(false);
  const mounted = useRef(true);
  const requestSeq = useRef(0);

  const load = useCallback(() => {
    if (!mounted.current) return;
    const seq = ++requestSeq.current;
    setLoading(true);
    Promise.all([uiConfig(), getDataQualityDashboard(hours)])
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

  const lagLabels = useMemo(
    () =>
      (data?.lag_series ?? []).map((row) => {
        const parsed = new Date(row.bucket);
        return Number.isNaN(parsed.getTime())
          ? row.bucket
          : parsed.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
      }),
    [data?.lag_series]
  );

  const parserLabels = useMemo(
    () =>
      (data?.parser_series ?? []).map((row) =>
        new Date(row.ts * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
      ),
    [data?.parser_series]
  );

  const consumerLabels = useMemo(
    () =>
      (data?.consumer_lag_series ?? []).map((row) =>
        new Date(row.ts * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
      ),
    [data?.consumer_lag_series]
  );

  return (
    <div className="page-grid data-quality-page">
      {err && <p className="error">{err}</p>}

      <DashboardToolbar
        title="Data quality"
        subtitle="Native trust layer for the SIEM pipeline: event completeness, ingest lag, parser quality and Redpanda consumer lag."
        hours={hours}
        autoRefreshSec={autoRefreshSec}
        loading={loading}
        onHoursChange={setHours}
        onAutoRefreshChange={setAutoRefreshSec}
        onRefresh={load}
      />

      <section className="card">
        <div className="btn-row">
          <Link className="tool-btn" to="/operations">
            Open operations center
          </Link>
          <Link className="tool-btn secondary" to="/events">
            Inspect events
          </Link>
          <a className="tool-btn secondary" href={config?.links.grafana || "#"} target="_blank" rel="noreferrer">
            Open Grafana
          </a>
        </div>
      </section>

      <section className="kpi-grid">
        <div className="kpi-card">
          <span>Total events</span>
          <strong>{formatCompact(data?.kpis.total_events)}</strong>
        </div>
        <div className="kpi-card">
          <span>Missing source IP</span>
          <strong>{data ? `${data.kpis.missing_source_ip_pct.toFixed(2)}%` : "—"}</strong>
        </div>
        <div className="kpi-card">
          <span>p95 ingest lag</span>
          <strong>{data ? `${data.kpis.p95_ingest_lag_ms.toFixed(0)} ms` : "—"}</strong>
        </div>
        <div className="kpi-card">
          <span>Unique source types</span>
          <strong>{formatCompact(data?.kpis.unique_source_types)}</strong>
        </div>
        <div className="kpi-card">
          <span>Parser ok / s</span>
          <strong>{formatCompact(data?.kpis.parser_ok_rate)}</strong>
        </div>
        <div className="kpi-card">
          <span>Parser error / s</span>
          <strong>{formatCompact(data?.kpis.parser_error_rate)}</strong>
        </div>
        <div className="kpi-card">
          <span>Consumer lag</span>
          <strong>{formatCompact(data?.kpis.consumer_lag)}</strong>
        </div>
      </section>

      <section className="dashboard-gauge-grid">
        <ObservabilityGaugePanel
          title="Parser success"
          value={
            data
              ? (data.kpis.parser_ok_rate / Math.max(data.kpis.parser_ok_rate + data.kpis.parser_error_rate, 1)) * 100
              : null
          }
          subtitle="Healthy parser throughput"
          formatter={formatPercent}
          kicker="Trust gauge"
          footer={<p className="meta stat-subtle">Green should dominate here; otherwise the rest of the console becomes untrustworthy fast.</p>}
        />
        <ObservabilityGaugePanel
          title="Freshness"
          value={data ? Math.max(0, 100 - (data.kpis.p95_ingest_lag_ms / 10_000) * 100) : null}
          subtitle="Lower lag means healthier data"
          formatter={formatPercent}
          kicker="Trust gauge"
          footer={<p className="meta stat-subtle">A compact translation of ingest lag into something operators can scan instantly.</p>}
        />
        <ObservabilityGaugePanel
          title="Source completeness"
          value={data ? Math.max(0, 100 - data.kpis.missing_source_ip_pct) : null}
          subtitle="Rows with source identity"
          formatter={formatPercent}
          kicker="Completeness gauge"
          footer={<p className="meta stat-subtle">Low completeness usually means hunting, attribution and pivoting degrade with it.</p>}
        />
        <ObservabilityGaugePanel
          title="Consumer readiness"
          value={data ? Math.max(0, 100 - Math.min(100, (data.kpis.consumer_lag / 5000) * 100)) : null}
          subtitle="Lag kept under control"
          formatter={formatPercent}
          kicker="Pipeline gauge"
          footer={<p className="meta stat-subtle">Backlog pressure is compressed into a single readiness number for fast validation.</p>}
        />
      </section>

      <section className="observability-grid">
        <ObservabilityBarPanel
          title="Pipeline trust summary"
          subtitle="Top trust risks compressed into one strip"
          rows={[
            { label: "missing source_ip %", value: data?.kpis.missing_source_ip_pct ?? 0, color: "#f0883e" },
            { label: "p95 ingest lag ms", value: data?.kpis.p95_ingest_lag_ms ?? 0, color: "#f85149" },
            { label: "parser error / s", value: data?.kpis.parser_error_rate ?? 0, color: "#8f6dff" },
            { label: "consumer lag", value: data?.kpis.consumer_lag ?? 0, color: "#4d9bff" },
          ]}
          valueFormatter={(value) => formatCompact(value)}
          axisFormatter={(value) => formatCompact(value)}
          kicker="Trust pane"
          footer={<p className="meta stat-subtle">This is the fastest operator view for why data might feel stale, incomplete or misleading.</p>}
        />

        <ObservabilityLinePanel
          title="Ingest lag by hour"
          subtitle={`Lag shown over ${data?.lag_window_hours ?? 24}h`}
          categories={lagLabels}
          series={[
            {
              name: "p95 ingest lag ms",
              color: "#f85149",
              data: (data?.lag_series ?? []).map((row) => row.p95_lag_ms),
              areaOpacity: 0.18,
            },
          ]}
          axisFormatter={(value) => formatCompact(value)}
          valueFormatter={(value) => `${formatCompact(value)} ms`}
          kicker="Lag pane"
          footer={<p className="meta stat-subtle">Sustained red growth here means data is landing too late for confident real-time triage.</p>}
        />
      </section>

      <section className="observability-grid">
        <ObservabilityLinePanel
          title="Parser quality"
          subtitle="Healthy parse flow versus parser errors"
          categories={parserLabels}
          series={[
            {
              name: "ok / s",
              color: "#7be37c",
              data: (data?.parser_series ?? []).map((row) => row.ok_rate),
              areaOpacity: 0.12,
            },
            {
              name: "error / s",
              color: "#f85149",
              data: (data?.parser_series ?? []).map((row) => row.error_rate),
            },
          ]}
          axisFormatter={(value) => formatCompact(value)}
          valueFormatter={(value) => formatCompact(value)}
          kicker="Parser pane"
          footer={<p className="meta stat-subtle">Green should dominate; rising red means malformed or degraded parser throughput.</p>}
        />

        <ObservabilityLinePanel
          title="Consumer lag trend"
          subtitle="Redpanda consumer backlog"
          categories={consumerLabels}
          series={[
            {
              name: "lag",
              color: "#4d9bff",
              data: (data?.consumer_lag_series ?? []).map((row) => row.lag),
              areaOpacity: 0.16,
            },
          ]}
          axisFormatter={(value) => formatCompact(value)}
          valueFormatter={(value) => formatCompact(value)}
          kicker="Backlog pane"
          footer={<p className="meta stat-subtle">Sustained spikes here usually mean the pipeline can no longer keep up with ingest.</p>}
        />
      </section>
    </div>
  );
}
