import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link } from "react-router-dom";
import { getOperationsDashboard, stackStatus, uiConfig, type OperationsDashboard, type StackStatus, type UiConfig } from "../api";
import DashboardToolbar from "../components/DashboardToolbar";
import { formatCompact, formatPercent } from "../dashboard-utils";
import {
  ObservabilityBarPanel,
  ObservabilityGaugePanel,
  ObservabilityLinePanel,
  ObservabilityPanel,
} from "../components/echarts/ObservabilityCharts";

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

  const clickhouseLabels = useMemo(
    () =>
      (data?.clickhouse_series ?? []).map((row) =>
        new Date(row.ts * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
      ),
    [data?.clickhouse_series]
  );

  const vectorLabels = useMemo(
    () =>
      (data?.vector_series ?? []).map((row) =>
        new Date(row.ts * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
      ),
    [data?.vector_series]
  );

  const pipelineLabels = useMemo(
    () =>
      (data?.pipeline_series ?? []).map((row) =>
        new Date(row.ts * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
      ),
    [data?.pipeline_series]
  );

  const healthyRatio = data ? (data.totals.healthy_components / Math.max(data.totals.total_components, 1)) * 100 : null;
  const vectorContinuity = data ? (data.totals.vector_forward_rate / Math.max(data.totals.vector_ingest_rate, 1)) * 100 : null;
  const pipelineContinuity = data ? (data.totals.detection_processed_rate / Math.max(data.totals.redpanda_records_rate, 1)) * 100 : null;

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

      <section className="dashboard-gauge-grid">
        <ObservabilityGaugePanel
          title="Service health"
          subtitle="Reachable platform targets"
          value={healthyRatio}
          formatter={(value) => formatPercent(value)}
          kicker="Availability gauge"
          footer={<p className="meta stat-subtle">Shows how much of the stack is actually reachable inside the platform loop.</p>}
        />
        <ObservabilityGaugePanel
          title="Vector continuity"
          subtitle="Forward versus ingest"
          value={vectorContinuity}
          formatter={(value) => formatPercent(value)}
          kicker="Pipeline gauge"
          footer={<p className="meta stat-subtle">Low continuity means ingest is arriving but not being forwarded cleanly.</p>}
        />
        <ObservabilityGaugePanel
          title="Detection continuity"
          subtitle="Processed versus topic records"
          value={pipelineContinuity}
          formatter={(value) => formatPercent(value)}
          kicker="Pipeline gauge"
          footer={<p className="meta stat-subtle">Shows whether the detection engine is keeping up with topic throughput.</p>}
        />
        <ObservabilityGaugePanel
          title="Alert pressure"
          subtitle="Firing alert saturation"
          value={data ? Math.min(100, data.totals.firing_alerts * 10) : null}
          formatter={() => formatCompact(data?.totals.firing_alerts)}
          kicker="Pressure gauge"
          footer={<p className="meta stat-subtle">A compact visual summary of how hard the response loop is being pushed right now.</p>}
        />
      </section>

      <section className="observability-grid">
        <ObservabilityLinePanel
          title="ClickHouse workload"
          subtitle="Query pressure across the storage layer"
          categories={clickhouseLabels}
          series={[
            {
              name: "select qps",
              color: "#4d9bff",
              data: (data?.clickhouse_series ?? []).map((row) => row.select_qps),
            },
            {
              name: "insert qps",
              color: "#7be37c",
              data: (data?.clickhouse_series ?? []).map((row) => row.insert_qps),
              areaOpacity: 0.12,
            },
            {
              name: "failed qps",
              color: "#f85149",
              data: (data?.clickhouse_series ?? []).map((row) => row.failed_qps),
            },
          ]}
          axisFormatter={(value) => formatCompact(value)}
          valueFormatter={(value) => formatCompact(value)}
          kicker="Storage pane"
          footer={<p className="meta stat-subtle">Red failed queries should stay flat; visible movement here is usually the first storage warning.</p>}
        />

        <ObservabilityLinePanel
          title="Vector flow"
          subtitle="Ingest and forward continuity"
          categories={vectorLabels}
          series={[
            {
              name: "http ingest",
              color: "#8f6dff",
              data: (data?.vector_series ?? []).map((row) => row.http_ingest_eps),
              areaOpacity: 0.14,
            },
            {
              name: "to redpanda",
              color: "#f0c15d",
              data: (data?.vector_series ?? []).map((row) => row.to_redpanda_eps),
            },
          ]}
          axisFormatter={(value) => formatCompact(value)}
          valueFormatter={(value) => formatCompact(value)}
          kicker="Collector pane"
          footer={
            <p className="meta stat-subtle">
              Flow continuity: {vectorContinuity != null ? formatPercent(vectorContinuity) : "—"}.
            </p>
          }
        />
      </section>

      <section className="observability-grid">
        <ObservabilityLinePanel
          title="Pipeline pressure"
          subtitle="Topic throughput versus detection processing"
          categories={pipelineLabels}
          series={[
            {
              name: "redpanda records",
              color: "#4d9bff",
              data: (data?.pipeline_series ?? []).map((row) => row.redpanda_records_eps),
              areaOpacity: 0.12,
            },
            {
              name: "detection processed",
              color: "#7be37c",
              data: (data?.pipeline_series ?? []).map((row) => row.detection_processed_eps),
            },
          ]}
          axisFormatter={(value) => formatCompact(value)}
          valueFormatter={(value) => formatCompact(value)}
          kicker="Pipeline pane"
          footer={<p className="meta stat-subtle">The gap between blue and green reveals whether the pipeline is accumulating unseen backlog.</p>}
        />

        <ObservabilityBarPanel
          title="Operational pressure points"
          subtitle="Fastest degradation indicators"
          rows={[
            { label: "parse errors 24h", value: data?.totals.parse_errors_24h ?? 0, color: "#f0883e" },
            { label: "dropped alerts 24h", value: data?.totals.dropped_alerts_24h ?? 0, color: "#f85149" },
            { label: "firing alerts", value: data?.totals.firing_alerts ?? 0, color: "#8f6dff" },
            { label: "parser in flight", value: data?.totals.parser_in_flight ?? 0, color: "#4d9bff" },
          ]}
          valueFormatter={(value) => formatCompact(value)}
          axisFormatter={(value) => formatCompact(value)}
          kicker="Pressure pane"
          footer={<p className="meta stat-subtle">These bars compress the highest-value operational alerts into one view for fast triage.</p>}
        />
      </section>

      <ObservabilityPanel
        title="Service status"
        subtitle="Reachability and latency for core services"
        kicker="Availability pane"
        footer={<p className="meta stat-subtle">Use this view to confirm whether missing data is a pipeline problem or just a charting symptom.</p>}
      >
        {!data?.component_status.length ? (
          <p className="meta">Prometheus did not return service status for the operations center.</p>
        ) : (
          <div className="infra-health-grid">
            {data.component_status.map((item) => (
              <div key={item.job} className={`health-card ${item.up ? "health-card-up" : "health-card-down"}`}>
                <div className="health-card-copy">
                  <strong>{item.job}</strong>
                  <small>{item.up ? "Target reachable" : "Target degraded"}</small>
                </div>
                <span className={`badge ${item.up ? "sev-low" : "sev-critical"}`}>{item.up ? "up" : "down"}</span>
              </div>
            ))}
          </div>
        )}
        {stack ? (
          <>
            <div className="section-divider" />
            <div className="property-grid ops-property-grid">
              {Object.entries(stack.components).map(([name, value]) => (
                <div key={name} className="property-card">
                  <span>{name}</span>
                  <strong>{value.ok ? "reachable" : "degraded"}</strong>
                  <small className="meta">{value.latency_ms ?? "—"} ms</small>
                </div>
              ))}
            </div>
          </>
        ) : null}
      </ObservabilityPanel>
    </div>
  );
}
