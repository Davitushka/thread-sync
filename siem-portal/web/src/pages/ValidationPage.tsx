import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link } from "react-router-dom";
import {
  getAlertsOverview,
  getCorrelatorStats,
  getDataQualityDashboard,
  getOperationsDashboard,
  getOverviewDashboard,
  stackStatus,
  uiConfig,
  type AlertsOverview,
  type CorrelatorStats,
  type DataQualityDashboard,
  type OperationsDashboard,
  type OverviewDashboard,
  type StackStatus,
  type UiConfig,
} from "../api";
import DashboardToolbar from "../components/DashboardToolbar";
import {
  ObservabilityBarPanel,
  ObservabilityGaugePanel,
  ObservabilityLinePanel,
} from "../components/echarts/ObservabilityCharts";
import { formatCompact, formatPercent } from "../dashboard-utils";

type ValidationState = "ok" | "warn" | "critical";

type ValidationCheck = {
  id: string;
  title: string;
  description: string;
  evidence: string;
  state: ValidationState;
  path: string;
  action: string;
};

function clampPct(value: number) {
  return Math.max(0, Math.min(value, 100));
}

function stateLabel(state: ValidationState) {
  if (state === "ok") return "Healthy";
  if (state === "warn") return "Warning";
  return "Critical";
}

export default function ValidationPage() {
  const [config, setConfig] = useState<UiConfig | null>(null);
  const [stack, setStack] = useState<StackStatus | null>(null);
  const [overview, setOverview] = useState<OverviewDashboard | null>(null);
  const [operations, setOperations] = useState<OperationsDashboard | null>(null);
  const [dataQuality, setDataQuality] = useState<DataQualityDashboard | null>(null);
  const [alerts, setAlerts] = useState<AlertsOverview | null>(null);
  const [correlator, setCorrelator] = useState<CorrelatorStats | null>(null);
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
    Promise.all([
      uiConfig(),
      stackStatus(),
      getOverviewDashboard(hours),
      getOperationsDashboard(hours),
      getDataQualityDashboard(hours),
      getAlertsOverview(),
      getCorrelatorStats(),
    ])
      .then(([cfg, stackData, overviewData, operationsData, qualityData, alertsData, correlatorData]) => {
        if (!mounted.current || seq !== requestSeq.current) return;
        setConfig(cfg);
        setStack(stackData);
        setOverview(overviewData);
        setOperations(operationsData);
        setDataQuality(qualityData);
        setAlerts(alertsData);
        setCorrelator(correlatorData);
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

  const serviceAvailabilityPct = useMemo(() => {
    if (!stack) return 0;
    const componentEntries = Object.values(stack.components);
    const healthy = componentEntries.filter((item) => item.ok).length;
    return clampPct((healthy / Math.max(componentEntries.length, 1)) * 100);
  }, [stack]);

  const parserSuccessPct = useMemo(() => {
    if (!dataQuality) return 0;
    const total = dataQuality.kpis.parser_ok_rate + dataQuality.kpis.parser_error_rate;
    return clampPct((dataQuality.kpis.parser_ok_rate / Math.max(total, 1)) * 100);
  }, [dataQuality]);

  const pipelineContinuityPct = useMemo(() => {
    if (!operations) return 0;
    return clampPct((operations.totals.detection_processed_rate / Math.max(operations.totals.redpanda_records_rate, 1)) * 100);
  }, [operations]);

  const freshnessPct = useMemo(() => {
    if (!dataQuality) return 0;
    return clampPct(100 - (dataQuality.kpis.p95_ingest_lag_ms / 10_000) * 100);
  }, [dataQuality]);

  const checks = useMemo<ValidationCheck[]>(() => {
    if (!stack || !overview || !operations || !dataQuality || !alerts || !correlator) return [];
    const componentEntries = Object.values(stack.components);
    const healthyComponents = componentEntries.filter((item) => item.ok).length;
    const vectorForwardPct = clampPct((operations.totals.vector_forward_rate / Math.max(operations.totals.vector_ingest_rate, 1)) * 100);
    const liveEventSignal = overview.kpis.total_events_24h > 0 || operations.totals.vector_ingest_rate > 0;
    const rulesLoaded = correlator.rules_count > 0;
    const signalVisible = alerts.totals.total > 0 || overview.kpis.critical_events_24h > 0;

    return [
      {
        id: "services",
        title: "Core services reachable",
        description: "Checks whether the portal can still see the core platform endpoints that daily dashboards depend on.",
        evidence: `${healthyComponents}/${componentEntries.length} services healthy`,
        state: serviceAvailabilityPct >= 95 ? "ok" : serviceAvailabilityPct >= 75 ? "warn" : "critical",
        path: "/operations",
        action: "Open operations",
      },
      {
        id: "ingest",
        title: "Event ingest is live",
        description: "Confirms the Vector to Redpanda path still produces fresh traffic and the overview is not stale.",
        evidence: `${formatCompact(operations.totals.vector_ingest_rate)} eps ingest, ${formatCompact(overview.kpis.total_events_24h)} events in range`,
        state: !liveEventSignal ? "critical" : vectorForwardPct >= 95 ? "ok" : vectorForwardPct >= 75 ? "warn" : "critical",
        path: "/",
        action: "Open overview",
      },
      {
        id: "parser",
        title: "Parser quality inside target",
        description: "Keeps parser success visibly dominant over malformed or failed parsing.",
        evidence: `${formatPercent(parserSuccessPct)} success, ${formatCompact(dataQuality.kpis.parser_error_rate)} errors/s`,
        state: parserSuccessPct >= 99 ? "ok" : parserSuccessPct >= 95 ? "warn" : "critical",
        path: "/data-quality",
        action: "Open data quality",
      },
      {
        id: "freshness",
        title: "Ingest freshness under control",
        description: "Surfaces whether data is landing quickly enough for the console to be trusted during triage.",
        evidence: `p95 ingest lag ${formatCompact(dataQuality.kpis.p95_ingest_lag_ms)} ms`,
        state: dataQuality.kpis.p95_ingest_lag_ms <= 2000 ? "ok" : dataQuality.kpis.p95_ingest_lag_ms <= 5000 ? "warn" : "critical",
        path: "/data-quality",
        action: "Inspect lag",
      },
      {
        id: "detection",
        title: "Detection engine ready",
        description: "Validates that rules are loaded and the detection path is still keeping up with incoming traffic.",
        evidence: `${formatCompact(correlator.rules_count)} rules, ${formatCompact(operations.totals.detection_processed_rate)} processed/s`,
        state: !rulesLoaded ? "critical" : pipelineContinuityPct >= 90 ? "ok" : pipelineContinuityPct >= 60 ? "warn" : "critical",
        path: "/detections",
        action: "Open detections",
      },
      {
        id: "alerts",
        title: "Alert signal visible in console",
        description: "Makes sure the operator can see produced alerts instead of relying on Grafana or Alertmanager tabs.",
        evidence: `${formatCompact(alerts.totals.total)} alerts, ${formatCompact(alerts.totals.active)} active`,
        state: signalVisible ? "ok" : overview.kpis.critical_events_24h > 0 ? "warn" : "ok",
        path: "/alerts",
        action: "Open alerts",
      },
    ];
  }, [
    alerts,
    correlator,
    dataQuality,
    operations,
    overview,
    pipelineContinuityPct,
    parserSuccessPct,
    serviceAvailabilityPct,
    stack,
  ]);

  const okChecks = checks.filter((check) => check.state === "ok").length;
  const warningChecks = checks.filter((check) => check.state === "warn").length;
  const criticalChecks = checks.filter((check) => check.state === "critical").length;
  const eventIntakeLabels = useMemo(
    () =>
      (overview?.events_per_minute ?? []).map((point) => {
        const parsed = new Date(point.minute);
        return Number.isNaN(parsed.getTime())
          ? point.minute
          : parsed.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
      }),
    [overview?.events_per_minute]
  );
  const parserTrendLabels = useMemo(
    () =>
      (dataQuality?.parser_series ?? []).map((row) =>
        new Date(row.ts * 1000).toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
      ),
    [dataQuality?.parser_series]
  );

  return (
    <div className="page-grid validation-page">
      {err && <p className="error">{err}</p>}

      <DashboardToolbar
        title="Validation workspace"
        subtitle="Native replacement for the Grafana validation dashboard: service reachability, ingest continuity, parser quality, data freshness and operator-visible signal."
        hours={hours}
        autoRefreshSec={autoRefreshSec}
        loading={loading}
        onHoursChange={setHours}
        onAutoRefreshChange={setAutoRefreshSec}
        onRefresh={load}
      />

      <section className="card workspace-pane">
        <div className="workspace-pane-header">
          <div className="workspace-pane-copy">
            <span className="workspace-pane-kicker">Validation lane</span>
            <h2>Native health and trust checks</h2>
            <p className="workspace-pane-subtitle">
              This workspace keeps the old Grafana validation intent, but moves it into the suite so operators can verify stack health without leaving the shell.
            </p>
          </div>
        </div>
        <div className="btn-row">
          <Link className="tool-btn" to="/operations">
            Open operations
          </Link>
          <Link className="tool-btn secondary" to="/data-quality">
            Open data quality
          </Link>
          <Link className="tool-btn secondary" to="/events">
            Open log explorer
          </Link>
          <a className="tool-btn secondary" href={config?.links.grafana || "#"} target="_blank" rel="noreferrer">
            Open Grafana fallback
          </a>
        </div>
        <div className="summary-grid">
          <div className="summary-card">
            <span>Healthy checks</span>
            <strong>{okChecks}/{checks.length || 6}</strong>
          </div>
          <div className="summary-card">
            <span>Warnings</span>
            <strong>{warningChecks}</strong>
          </div>
          <div className="summary-card">
            <span>Criticals</span>
            <strong>{criticalChecks}</strong>
          </div>
          <div className="summary-card">
            <span>Events in range</span>
            <strong>{formatCompact(overview?.kpis.total_events_24h)}</strong>
          </div>
          <div className="summary-card">
            <span>Parser success</span>
            <strong>{formatPercent(parserSuccessPct)}</strong>
          </div>
          <div className="summary-card">
            <span>p95 ingest lag</span>
            <strong>{dataQuality ? `${formatCompact(dataQuality.kpis.p95_ingest_lag_ms)} ms` : "—"}</strong>
          </div>
          <div className="summary-card">
            <span>Rules loaded</span>
            <strong>{formatCompact(correlator?.rules_count)}</strong>
          </div>
          <div className="summary-card">
            <span>Visible alerts</span>
            <strong>{formatCompact(alerts?.totals.total)}</strong>
          </div>
        </div>
      </section>

      <section className="dashboard-gauge-grid">
        <ObservabilityGaugePanel
          title="Service availability"
          value={serviceAvailabilityPct}
          subtitle="Reachable platform endpoints"
          formatter={(value) => formatPercent(value)}
          kicker="Validation gauge"
        />
        <ObservabilityGaugePanel
          title="Parser success"
          value={parserSuccessPct}
          subtitle="Successful parser throughput"
          formatter={(value) => formatPercent(value)}
          kicker="Validation gauge"
        />
        <ObservabilityGaugePanel
          title="Pipeline continuity"
          value={pipelineContinuityPct}
          subtitle="Processed versus produced traffic"
          formatter={(value) => formatPercent(value)}
          kicker="Validation gauge"
        />
        <ObservabilityGaugePanel
          title="Data freshness"
          value={freshnessPct}
          subtitle="Fresh data landing inside target"
          formatter={(value) => formatPercent(value)}
          kicker="Validation gauge"
        />
      </section>

      <section className="observability-grid">
        <ObservabilityLinePanel
          title="Event intake continuity"
          subtitle="Dead or delayed ingest should stand out immediately"
          categories={eventIntakeLabels}
          series={[
            {
              name: "events",
              color: "#7be37c",
              data: (overview?.events_per_minute ?? []).map((point) => point.events),
              areaOpacity: 0.18,
            },
          ]}
          axisFormatter={(value) => formatCompact(value)}
          valueFormatter={(value) => formatCompact(value)}
          kicker="Signal pane"
          footer={
            <p className="meta stat-subtle">
              Vector ingest now: {formatCompact(operations?.totals.vector_ingest_rate)} eps. Detection throughput: {formatCompact(operations?.totals.detection_processed_rate)} /s.
            </p>
          }
        />

        <ObservabilityLinePanel
          title="Parser quality trend"
          subtitle="Green should dominate if the suite is trustworthy"
          categories={parserTrendLabels}
          series={[
            {
              name: "ok / s",
              color: "#7be37c",
              data: (dataQuality?.parser_series ?? []).map((row) => row.ok_rate),
              areaOpacity: 0.12,
            },
            {
              name: "error / s",
              color: "#f85149",
              data: (dataQuality?.parser_series ?? []).map((row) => row.error_rate),
            },
          ]}
          axisFormatter={(value) => formatCompact(value)}
          valueFormatter={(value) => formatCompact(value)}
          kicker="Quality pane"
          footer={<p className="meta stat-subtle">Consumer lag: {formatCompact(dataQuality?.kpis.consumer_lag)}. Rising red together with lag means the pipeline is degrading.</p>}
        />
      </section>

      <section className="infra-grid">
        <article className="card workspace-pane">
          <div className="workspace-pane-header">
            <div className="workspace-pane-copy">
              <span className="workspace-pane-kicker">Checklist pane</span>
              <h2>Operator validation checklist</h2>
              <p className="workspace-pane-subtitle">Fast, opinionated checks that answer the same question analysts usually ask Grafana: can this suite be trusted right now?</p>
            </div>
          </div>
          <div className="validation-check-grid">
            {checks.map((check) => (
              <div key={check.id} className={`validation-check-card validation-check-${check.state}`}>
                <div className="validation-check-head">
                  <strong>{check.title}</strong>
                  <span className={`validation-status validation-status-${check.state}`}>{stateLabel(check.state)}</span>
                </div>
                <p>{check.description}</p>
                <div className="validation-evidence">{check.evidence}</div>
                <Link className="validation-action" to={check.path}>
                  {check.action}
                </Link>
              </div>
            ))}
          </div>
        </article>

        <article className="card workspace-pane">
          <div className="workspace-pane-header">
            <div className="workspace-pane-copy">
              <span className="workspace-pane-kicker">Pressure pane</span>
              <h2>Validation pressure points</h2>
              <p className="workspace-pane-subtitle">Dense indicators that usually explain why dashboards go stale, empty or misleading.</p>
            </div>
          </div>
          <ObservabilityBarPanel
            title="Validation pressure points"
            subtitle="High-value indicators for stale or misleading dashboards"
            rows={[
              { label: "service failures", value: Math.max(0, (operations?.totals.total_components ?? 0) - (operations?.totals.healthy_components ?? 0)), color: "#f85149" },
              { label: "p95 lag ms", value: dataQuality?.kpis.p95_ingest_lag_ms ?? 0, color: "#f0883e" },
              { label: "parser errors / s", value: dataQuality?.kpis.parser_error_rate ?? 0, color: "#d29922" },
              { label: "active alerts", value: alerts?.totals.active ?? 0, color: "#8f6dff" },
              { label: "pending forwards", value: correlator?.pending_alerts ?? 0, color: "#4d9bff" },
            ]}
            valueFormatter={(value) => formatCompact(value)}
            axisFormatter={(value) => formatCompact(value)}
            kicker="Pressure pane"
            height={280}
          />
          <div className="section-divider" />
          <div className="infra-health-grid">
            {stack
              ? Object.entries(stack.components).map(([name, value]) => (
                  <div key={name} className="health-card">
                    <strong>{name}</strong>
                    <span className={`badge ${value.ok ? "sev-low" : "sev-critical"}`}>{value.ok ? "up" : "down"}</span>
                  </div>
                ))
              : null}
          </div>
        </article>
      </section>
    </div>
  );
}
