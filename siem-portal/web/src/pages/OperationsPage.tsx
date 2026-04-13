import { useCallback, useEffect, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { getOperationsDashboard, stackStatus, uiConfig, type OperationsDashboard, type StackStatus, type UiConfig } from "../api";
import DashboardToolbar from "../components/DashboardToolbar";
import { NativeBarChart, NativeMultiLineChart } from "../components/NativeCharts";
import { formatCompact, formatPercent } from "../dashboard-utils";

export default function OperationsPage() {
  const [config, setConfig] = useState<UiConfig | null>(null);
  const [stack, setStack] = useState<StackStatus | null>(null);
  const [data, setData] = useState<OperationsDashboard | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [hours, setHours] = useState(24);
  const [autoRefreshSec, setAutoRefreshSec] = useState(30);
  const [loading, setLoading] = useState(false);
  const mounted = useRef(true);
  const requestSeq = useRef(0);

  const load = useCallback(() => {
    if (!mounted.current) return;
    const seq = ++requestSeq.current;
    setLoading(true);
    Promise.all([uiConfig(), stackStatus(), getOperationsDashboard(hours)])
      .then(([cfg, stackData, dashboard]) => {
        if (!mounted.current || seq !== requestSeq.current) return;
        setConfig(cfg);
        setStack(stackData);
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
    <div className="page-grid operations-page">
      {err && <p className="error">{err}</p>}

      <DashboardToolbar
        title="Operations center"
        subtitle="Native operational visibility for the SIEM pipeline: service uptime, ClickHouse workload, Vector flow, Redpanda throughput and detection pressure."
        hours={hours}
        autoRefreshSec={autoRefreshSec}
        loading={loading}
        onHoursChange={setHours}
        onAutoRefreshChange={setAutoRefreshSec}
        onRefresh={load}
      />

      <section className="card">
        <div className="btn-row">
          <Link className="tool-btn" to="/data-quality">
            Open data quality
          </Link>
          <Link className="tool-btn secondary" to="/dashboards">
            Back to dashboards hub
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
          <span>Healthy components</span>
          <strong>
            {data ? `${data.totals.healthy_components}/${data.totals.total_components}` : "—"}
          </strong>
        </div>
        <div className="kpi-card">
          <span>CH select qps</span>
          <strong>{formatCompact(data?.totals.clickhouse_select_qps)}</strong>
        </div>
        <div className="kpi-card">
          <span>CH insert qps</span>
          <strong>{formatCompact(data?.totals.clickhouse_insert_qps)}</strong>
        </div>
        <div className="kpi-card">
          <span>Vector ingest eps</span>
          <strong>{formatCompact(data?.totals.vector_ingest_rate)}</strong>
        </div>
        <div className="kpi-card">
          <span>Redpanda records/s</span>
          <strong>{formatCompact(data?.totals.redpanda_records_rate)}</strong>
        </div>
        <div className="kpi-card">
          <span>Detection processed/s</span>
          <strong>{formatCompact(data?.totals.detection_processed_rate)}</strong>
        </div>
        <div className="kpi-card">
          <span>Firing alerts</span>
          <strong>{formatCompact(data?.totals.firing_alerts)}</strong>
        </div>
        <div className="kpi-card">
          <span>Parser in flight</span>
          <strong>{formatCompact(data?.totals.parser_in_flight)}</strong>
        </div>
      </section>

      <section className="infra-grid">
        <article className="card">
          <h2>ClickHouse workload</h2>
          {!data?.clickhouse_series.length ? (
            <p className="meta">Нет рядов по ClickHouse workload за выбранный диапазон.</p>
          ) : (
            <>
              <NativeMultiLineChart
                title="ClickHouse workload"
                points={data.clickhouse_series.map((row) => ({
                  x: String(row.ts),
                  select_qps: row.select_qps,
                  insert_qps: row.insert_qps,
                  failed_qps: row.failed_qps,
                }))}
                series={[
                  { key: "select_qps", label: "select qps", color: "#4d9bff" },
                  { key: "insert_qps", label: "insert qps", color: "#7be37c" },
                  { key: "failed_qps", label: "failed qps", color: "#f85149" },
                ]}
              />
              <p className="meta stat-subtle">Current failed query rate is shown as red to surface storage pressure quickly.</p>
            </>
          )}
        </article>

        <article className="card">
          <h2>Vector flow</h2>
          {!data?.vector_series.length ? (
            <p className="meta">Нет рядов по Vector ingest/forward потоку.</p>
          ) : (
            <>
              <NativeMultiLineChart
                title="Vector flow"
                points={data.vector_series.map((row) => ({
                  x: String(row.ts),
                  http_ingest_eps: row.http_ingest_eps,
                  to_redpanda_eps: row.to_redpanda_eps,
                }))}
                series={[
                  { key: "http_ingest_eps", label: "http ingest", color: "#8f6dff" },
                  { key: "to_redpanda_eps", label: "to redpanda", color: "#f0c15d" },
                ]}
              />
              <p className="meta stat-subtle">
                Flow continuity: {data ? formatPercent((data.totals.vector_forward_rate / Math.max(data.totals.vector_ingest_rate, 1)) * 100) : "—"}
              </p>
            </>
          )}
        </article>
      </section>

      <section className="infra-grid">
        <article className="card">
          <h2>Pipeline pressure</h2>
          {!data?.pipeline_series.length ? (
            <p className="meta">Нет pipeline throughput метрик за диапазон.</p>
          ) : (
            <>
              <NativeMultiLineChart
                title="Pipeline pressure"
                points={data.pipeline_series.map((row) => ({
                  x: String(row.ts),
                  redpanda_records_eps: row.redpanda_records_eps,
                  detection_processed_eps: row.detection_processed_eps,
                }))}
                series={[
                  { key: "redpanda_records_eps", label: "redpanda records", color: "#4d9bff" },
                  { key: "detection_processed_eps", label: "detection processed", color: "#7be37c" },
                ]}
              />
              <p className="meta stat-subtle">Blue = ingestion into the topic, green = detection engine processing speed.</p>
            </>
          )}
        </article>

        <article className="card">
          <h2>Operational pressure points</h2>
          <NativeBarChart
            title="Operational pressure points"
            rows={[
              { label: "parse errors 24h", value: data?.totals.parse_errors_24h ?? 0, tone: "#f0883e" },
              { label: "dropped alerts 24h", value: data?.totals.dropped_alerts_24h ?? 0, tone: "#f85149" },
              { label: "firing alerts", value: data?.totals.firing_alerts ?? 0, tone: "#8f6dff" },
              { label: "parser in flight", value: data?.totals.parser_in_flight ?? 0, tone: "#4d9bff" },
            ]}
            valueFormatter={(value) => formatCompact(value)}
          />
          <p className="meta stat-subtle">These are the fastest indicators that the pipeline is degraded or under unusual stress.</p>
        </article>
      </section>

      <section className="card">
        <h2>Service status</h2>
        {!data?.component_status.length ? (
          <p className="meta">Prometheus не вернул service status для Operations center.</p>
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
        {stack ? (
          <div className="property-grid ops-property-grid">
            {Object.entries(stack.components).map(([name, value]) => (
              <div key={name} className="property-card">
                <span>{name}</span>
                <strong>{value.ok ? "reachable" : "degraded"}</strong>
                <small className="meta">{value.latency_ms ?? "—"} ms</small>
              </div>
            ))}
          </div>
        ) : null}
      </section>
    </div>
  );
}
