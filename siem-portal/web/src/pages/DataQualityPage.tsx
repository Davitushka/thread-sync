import { useCallback, useEffect, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { getDataQualityDashboard, uiConfig, type DataQualityDashboard, type UiConfig } from "../api";
import DashboardToolbar from "../components/DashboardToolbar";
import { NativeBarChart, NativeLineChart, NativeMultiLineChart } from "../components/NativeCharts";
import { formatCompact } from "../dashboard-utils";

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

      <section className="infra-grid">
        <article className="card">
          <h2>Pipeline trust summary</h2>
          <NativeBarChart
            title="Pipeline trust summary"
            rows={[
              { label: "missing source_ip %", value: data?.kpis.missing_source_ip_pct ?? 0, tone: "#f0883e" },
              { label: "p95 ingest lag ms", value: data?.kpis.p95_ingest_lag_ms ?? 0, tone: "#f85149" },
              { label: "parser error / s", value: data?.kpis.parser_error_rate ?? 0, tone: "#8f6dff" },
              { label: "consumer lag", value: data?.kpis.consumer_lag ?? 0, tone: "#4d9bff" },
            ]}
            valueFormatter={(value) => formatCompact(value)}
          />
          <p className="meta stat-subtle">This compresses the top trust risks into a single visual strip for fast triage.</p>
        </article>

        <article className="card">
          <h2>Ingest lag by hour</h2>
          {!data?.lag_series.length ? (
            <p className="meta">Нет lag timeline по ClickHouse за выбранное окно.</p>
          ) : (
            <>
              <NativeLineChart
                title="Lag ingest by hour"
                color="#f85149"
                points={data.lag_series.map((row) => ({ x: row.bucket, y: row.p95_lag_ms }))}
              />
              <p className="meta stat-subtle">The lag series is shown over {data?.lag_window_hours ?? 24}h to reveal delayed ingestion patterns.</p>
            </>
          )}
        </article>
      </section>

      <section className="infra-grid">
        <article className="card">
          <h2>Parser quality</h2>
          {!data?.parser_series.length ? (
            <p className="meta">Нет parser quality series за выбранный диапазон.</p>
          ) : (
            <>
              <NativeMultiLineChart
                title="Parser quality"
                points={data.parser_series.map((row) => ({
                  x: String(row.ts),
                  ok_rate: row.ok_rate,
                  error_rate: row.error_rate,
                }))}
                series={[
                  { key: "ok_rate", label: "ok / s", color: "#7be37c" },
                  { key: "error_rate", label: "error / s", color: "#f85149" },
                ]}
              />
              <p className="meta stat-subtle">Green should dominate; rising red means malformed or degraded parser throughput.</p>
            </>
          )}
        </article>

        <article className="card">
          <h2>Consumer lag trend</h2>
          {!data?.consumer_lag_series.length ? (
            <p className="meta">Нет Redpanda consumer lag series.</p>
          ) : (
            <>
              <NativeLineChart
                title="Consumer lag trend"
                color="#4d9bff"
                points={data.consumer_lag_series.map((row) => ({ x: String(row.ts), y: row.lag }))}
              />
              <p className="meta stat-subtle">Sustained spikes here usually mean the pipeline can no longer keep up with ingest.</p>
            </>
          )}
        </article>
      </section>
    </div>
  );
}
