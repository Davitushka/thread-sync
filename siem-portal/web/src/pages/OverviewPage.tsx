import { useCallback, useEffect, useMemo, useRef, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
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
import {
  ObservabilityBarPanel,
  ObservabilityGaugePanel,
  ObservabilityLinePanel,
  ObservabilityPanel,
} from "../components/echarts/ObservabilityCharts";
import { useWorkspaceShell } from "../components/WorkspaceShellContext";
import { formatCompact, formatPercent, shortDateTime } from "../dashboard-utils";

export default function OverviewPage() {
  const { openOrFocusWorkspace } = useWorkspaceShell();
  const navigate = useNavigate();
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

  const eventsLabels = useMemo(
    () =>
      (overview?.events_per_minute ?? []).map((point) => {
        const parsed = new Date(point.minute);
        return Number.isNaN(parsed.getTime())
          ? point.minute
          : parsed.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
      }),
    [overview?.events_per_minute]
  );

  const severityTimelineLabels = useMemo(
    () =>
      (overview?.severity_timeline ?? []).map((row) => {
        const parsed = new Date(row.bucket);
        return Number.isNaN(parsed.getTime())
          ? row.bucket
          : parsed.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" });
      }),
    [overview?.severity_timeline]
  );
  const eventsSeries = useMemo(() => {
    const values = (overview?.events_per_minute ?? []).map((point) => point.events);
    if (values.length) return values;
    return [overview?.kpis.total_events_24h ?? 0];
  }, [overview]);
  const eventsCategories = useMemo(() => {
    if (eventsLabels.length) return eventsLabels;
    return ["now"];
  }, [eventsLabels]);
  const severityRows = useMemo(() => {
    const rows = (overview?.severity_breakdown ?? []).map((row) => ({
      label: row.severity,
      value: row.events,
      color:
        row.severity === "critical"
          ? "#f85149"
          : row.severity === "error"
            ? "#f0883e"
            : row.severity === "warning"
              ? "#d29922"
              : "#3fb950",
    }));
    if (rows.length) return rows;
    const critical = overview?.kpis.critical_events_24h ?? 0;
    const total = overview?.kpis.total_events_24h ?? 0;
    return [
      { label: "critical", value: critical, color: "#f85149" },
      { label: "other", value: Math.max(0, total - critical), color: "#4d9bff" },
    ];
  }, [overview]);
  const severityTimelineSeries = useMemo(() => {
    const source = overview?.severity_timeline ?? [];
    if (source.length) {
      return {
        categories: severityTimelineLabels,
        critical: source.map((row) => row.critical),
        error: source.map((row) => row.error),
        warning: source.map((row) => row.warning),
      };
    }
    const critical = overview?.kpis.critical_events_24h ?? 0;
    const total = overview?.kpis.total_events_24h ?? 0;
    const errorAndCritical = Math.round(((overview?.kpis.error_pct_24h ?? 0) / 100) * total);
    const errorOnly = Math.max(0, errorAndCritical - critical);
    return {
      categories: ["now"],
      critical: [critical],
      error: [errorOnly],
      warning: [Math.max(0, total - critical - errorOnly)],
    };
  }, [overview, severityTimelineLabels]);
  const topIpRows = useMemo(() => {
    const rows = (overview?.top_source_ips ?? []).map((row) => ({
      label: row.source_ip,
      value: row.events,
      color: row.threats > 0 ? "#f85149" : "#4d9bff",
    }));
    if (rows.length) return rows;
    return (overview?.source_breakdown ?? []).slice(0, 6).map((row) => ({
      label: `src:${row.source_type}`,
      value: row.events,
      color: "#4d9bff",
    }));
  }, [overview]);
  const sourceRows = useMemo(() => {
    const rows = (overview?.source_breakdown ?? []).map((row) => ({
      label: row.source_type,
      value: row.events,
      color: "#4d9bff",
    }));
    if (rows.length) return rows;
    return [{ label: "events", value: overview?.kpis.total_events_24h ?? 0, color: "#4d9bff" }];
  }, [overview]);

  const stackRows = useMemo(
    () =>
      stack
        ? Object.entries(stack.components).map(([name, value]) => ({
            label: name,
            value: value.ok ? 1 : 0,
            color: value.ok ? "#7be37c" : "#f85149",
          }))
        : [],
    [stack]
  );
  const openEventsPivot = useCallback(
    (params: Record<string, string>) => {
      const query = new URLSearchParams(params).toString();
      openOrFocusWorkspace("/events");
      navigate(query ? `/events?${query}` : "/events");
    },
    [navigate, openOrFocusWorkspace]
  );

  return (
    <div className="page-grid overview-dashboard">
      {err && <p className="error">{err}</p>}
      <DashboardToolbar
        title="SOC overview"
        subtitle="Native command surface over ClickHouse and platform services, with operational pivots and no iframe-first workflow."
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
        <div className="workspace-pane-header">
          <div className="workspace-pane-copy">
            <span className="workspace-pane-kicker">Mission control actions</span>
            <h2>Primary operator pivots</h2>
            <p className="workspace-pane-subtitle">Jump into the most common daily workflows without leaving the overview surface.</p>
          </div>
        </div>
        <div className="btn-row">
          <Link className="tool-btn secondary" to="/infrastructure">
            Open infrastructure
          </Link>
          <Link className="tool-btn secondary" to="/operations">
            Open operations center
          </Link>
          <Link className="tool-btn secondary" to="/data-quality">
            Open data quality
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

      <section className="dashboard-gauge-grid">
        <ObservabilityGaugePanel
          title="Critical pressure"
          subtitle="Critical signal share"
          value={
            overview
              ? (overview.kpis.critical_events_24h / Math.max(overview.kpis.total_events_24h, 1)) * 100
              : null
          }
          formatter={formatPercent}
          kicker="Risk gauge"
          footer={<p className="meta stat-subtle">Shows how much of the event stream is already in the most dangerous slice.</p>}
        />
        <ObservabilityGaugePanel
          title="Error + critical"
          subtitle="Escalation pressure"
          value={overview?.kpis.error_pct_24h}
          formatter={(value) => `${value.toFixed(2)}%`}
          kicker="Risk gauge"
          footer={<p className="meta stat-subtle">Higher values usually mean the analyst console should shift from posture to triage mode.</p>}
        />
        <ObservabilityGaugePanel
          title="Alert visibility"
          subtitle="Active alerts present"
          value={alertsCount != null ? Math.min(100, alertsCount * 10) : null}
          formatter={() => formatCompact(alertsCount)}
          kicker="Queue gauge"
          footer={<p className="meta stat-subtle">A fast signal that the upstream detection and alerting flow is surfacing into the suite.</p>}
        />
        <ObservabilityGaugePanel
          title="Case pressure"
          subtitle="Open investigation load"
          value={casesCount != null ? Math.min(100, casesCount * 10) : null}
          formatter={() => formatCompact(casesCount)}
          kicker="Queue gauge"
          footer={<p className="meta stat-subtle">Helps balance current alert volume against ongoing case workload.</p>}
        />
      </section>

      <section className="observability-grid observability-grid-primary">
        <ObservabilityLinePanel
          title="Events timeline"
          subtitle="Volume trend for the selected window"
          categories={eventsCategories}
          series={[
            {
              name: "events",
              color: "#7be37c",
              data: eventsSeries,
              areaOpacity: 0.22,
            },
          ]}
          axisFormatter={(value) => formatCompact(value)}
          valueFormatter={(value) => formatCompact(value)}
          kicker="Signal pane"
          className="observability-panel-wide"
          showDataZoom
          onPointClick={({ dataIndex }) => {
            const row = overview?.events_per_minute[dataIndex];
            if (!row) return;
            const start = new Date(row.minute);
            if (Number.isNaN(start.getTime())) {
              openEventsPivot({ q: row.minute });
              return;
            }
            const end = new Date(start.getTime() + (overview?.bucket_minutes ?? 1) * 60_000);
            openEventsPivot({ start: start.toISOString(), end: end.toISOString() });
          }}
          footer={
            <p className="meta stat-subtle">
              Bucket = {overview?.bucket_minutes ?? 1} min, range = {overview?.window_hours ?? hours}h.
            </p>
          }
        />

        <ObservabilityBarPanel
          title="Events by severity"
          subtitle="Criticality split for the current horizon"
          rows={severityRows}
          valueFormatter={(value) => formatCompact(value)}
          axisFormatter={(value) => formatCompact(value)}
          kicker="Distribution pane"
          onRowClick={({ label }) => openEventsPivot({ severity: label })}
          footer={<p className="meta stat-subtle">This gives a fast read on whether the stream is mostly background noise or operator-relevant pressure.</p>}
        />
      </section>

      <ObservabilityLinePanel
        title="Severity timeline"
        subtitle="Rolling severity pressure across the current window"
        categories={severityTimelineSeries.categories}
        series={[
          {
            name: "critical",
            color: "#f85149",
            data: severityTimelineSeries.critical,
            areaOpacity: 0.14,
          },
          {
            name: "error",
            color: "#f0883e",
            data: severityTimelineSeries.error,
          },
          {
            name: "warning",
            color: "#d29922",
            data: severityTimelineSeries.warning,
          },
        ]}
        axisFormatter={(value) => formatCompact(value)}
        valueFormatter={(value) => formatCompact(value)}
        kicker="Trend pane"
        showDataZoom
        footer={<p className="meta stat-subtle">Use this to distinguish sustained escalation from short bursts of noisy traffic.</p>}
      />

      <section className="observability-grid">
        <ObservabilityBarPanel
          title="Top source IPs"
          subtitle="High-traffic and high-threat sources"
          rows={topIpRows}
          valueFormatter={(value) => formatCompact(value)}
          axisFormatter={(value) => formatCompact(value)}
          kicker="Exposure pane"
          onRowClick={({ label }) => openEventsPivot({ source_ip: label })}
          footer={<p className="meta stat-subtle">Red bars indicate sources that also produced threat-tagged activity in the selected range.</p>}
        />

        <article className="card workspace-pane">
          <div className="workspace-pane-header">
            <div className="workspace-pane-copy">
              <span className="workspace-pane-kicker">Activity pane</span>
              <h2>Recent security events</h2>
              <p className="workspace-pane-subtitle">Fresh security-relevant activity that can be opened directly into hunt workflows.</p>
            </div>
          </div>
          {!overview?.recent_security_events.length ? (
            <p className="meta">Нет recent security events в `siem.events`.</p>
          ) : (
            <div className="recent-event-stack">
              {overview.recent_security_events.slice(0, 8).map((row) => (
                <button
                  type="button"
                  key={row.event_id}
                  className="recent-event-card"
                  onClick={() =>
                    openEventsPivot(
                      row.source_ip
                        ? { source_ip: row.source_ip }
                        : row.host
                          ? { host: row.host }
                          : { source_type: row.source_type, q: row.message.slice(0, 80) }
                    )
                  }
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

      <section className="observability-grid">
        <ObservabilityBarPanel
          title="Top sources"
          subtitle="Source-type contribution to current traffic"
          rows={sourceRows}
          valueFormatter={(value) => formatCompact(value)}
          axisFormatter={(value) => formatCompact(value)}
          kicker="Coverage pane"
          onRowClick={({ label }) => openEventsPivot({ source_type: label })}
          footer={<p className="meta stat-subtle">Highlights which ingest families dominate the current operator view.</p>}
        />

        <ObservabilityPanel
          title="Stack health"
          subtitle="Fast reachability readout for core services"
          kicker="Platform pane"
          footer={<p className="meta stat-subtle">Use this before jumping into deeper platform views or deciding whether the data can be trusted.</p>}
        >
          {!stack ? (
            <p className="meta">Loading stack status…</p>
          ) : (
            <>
              <div className="infra-health-grid">
                {Object.entries(stack.components).map(([name, value]) => (
                  <div key={name} className={`health-card ${value.ok ? "health-card-up" : "health-card-down"}`}>
                    <div className="health-card-copy">
                      <strong>{name}</strong>
                      <small>{value.latency_ms ?? "—"} ms</small>
                    </div>
                    <span className={`badge ${value.ok ? "sev-low" : "sev-critical"}`}>{value.ok ? "ok" : "down"}</span>
                  </div>
                ))}
              </div>
              <div className="section-divider" />
              <ObservabilityBarPanel
                title="Service availability"
                subtitle="Healthy versus degraded targets"
                rows={stackRows}
                valueFormatter={(value) => (value > 0 ? "healthy" : "down")}
                axisFormatter={(value) => `${Math.round(value)}`}
                kicker="Platform pane"
                className="observability-panel-embedded"
                height={220}
              />
            </>
          )}
        </ObservabilityPanel>
      </section>

      <section className="card workspace-pane">
        <div className="workspace-pane-header">
          <div className="workspace-pane-copy">
            <span className="workspace-pane-kicker">Deep-dive pane</span>
            <h2>Deep-dive surfaces</h2>
            <p className="workspace-pane-subtitle">Engineering and analytics surfaces kept nearby for validation and advanced analysis.</p>
          </div>
        </div>
        <div className="home-grid">
          <Link className="home-card" to="/infrastructure">
            <h2>Native infrastructure</h2>
            <p>Host, network, container and component health in a native operator workflow.</p>
          </Link>
          <Link className="home-card" to="/dashboards">
            <h2>Analytics hub</h2>
            <p>Curated daily dashboards in the suite, with Grafana reserved for engineering deep-dives.</p>
          </Link>
          <a className="home-card" href={config?.links.grafana || "#"} target="_blank" rel="noreferrer">
            <h2>Grafana</h2>
            <p>Explore mode, engineering panels and raw deep-dive views for ClickHouse, Loki and service internals.</p>
          </a>
          <a
            className="home-card"
            href={config?.links.siem_overview_dashboard || config?.links.grafana || "#"}
            target="_blank"
            rel="noreferrer"
          >
            <h2>Reference overview</h2>
            <p>Legacy Grafana overview kept as a comparison and fallback surface during native migration.</p>
          </a>
          <a className="home-card" href={config?.links.prometheus || "#"} target="_blank" rel="noreferrer">
            <h2>Prometheus</h2>
            <p>PromQL validation, target health and engineering metrics for deep operational validation.</p>
          </a>
        </div>
      </section>
    </div>
  );
}
