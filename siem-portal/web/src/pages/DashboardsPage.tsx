import { useEffect, useMemo, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { uiConfig, type UiConfig } from "../api";
import AdaptivePaneLayout from "../components/AdaptivePaneLayout";
import { ObservabilityGaugePanel, ObservabilityLinePanel } from "../components/echarts/ObservabilityCharts";
import {
  DASHBOARDS,
  DASHBOARD_GROUPS,
  DASHBOARD_TIME_RANGES,
  grafanaDashboardUrl,
  type DashboardEntry,
} from "../dashboard-catalog";
import { formatCompact } from "../dashboard-utils";

export default function DashboardsPage() {
  const navigate = useNavigate();
  const [config, setConfig] = useState<UiConfig | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [group, setGroup] = useState<(typeof DASHBOARD_GROUPS)[number]>("SOC Core");
  const [selectedId, setSelectedId] = useState<string>("overview");
  const [timeRange, setTimeRange] = useState<string>("now-24h");

  useEffect(() => {
    uiConfig()
      .then(setConfig)
      .catch((e) => setErr(String(e)));
  }, []);

  const items = useMemo(() => DASHBOARDS.filter((item) => item.group === group), [group]);

  useEffect(() => {
    if (!items.some((item) => item.id === selectedId)) {
      setSelectedId(items[0]?.id ?? "overview");
    }
  }, [items, selectedId]);

  const current = useMemo(
    () => DASHBOARDS.find((item) => item.id === selectedId) ?? DASHBOARDS[0],
    [selectedId]
  );
  const grafanaRoot = config?.links.grafana || "";
  const embedUrl =
    grafanaRoot && current.kind === "grafana" && current.uid ? grafanaDashboardUrl(grafanaRoot, current.uid, timeRange, true) : "";
  const openUrl =
    grafanaRoot && current.kind === "grafana" && current.uid ? grafanaDashboardUrl(grafanaRoot, current.uid, timeRange, false) : "";
  const nativeCount = DASHBOARDS.filter((item) => item.kind === "native").length;
  const grafanaCount = DASHBOARDS.filter((item) => item.kind === "grafana").length;
  const hybridCount = DASHBOARDS.filter((item) => item.status === "hybrid").length;
  const dailyCount = DASHBOARDS.filter((item) => item.priority === "daily").length;
  const supportCount = DASHBOARDS.filter((item) => item.priority === "support").length;
  const deepDiveCount = DASHBOARDS.filter((item) => item.priority === "deep-dive").length;
  const primaryItems = items.filter((item) => item.priority === "daily" || item.priority === "support");
  const secondaryItems = items.filter((item) => item.priority === "bridge" || item.priority === "deep-dive");
  const migrationCoverage = Math.round((nativeCount / Math.max(DASHBOARDS.length, 1)) * 100);
  const dailyNativeCoverage = Math.round(
    (DASHBOARDS.filter((item) => item.priority !== "deep-dive" && item.kind === "native").length /
      Math.max(DASHBOARDS.filter((item) => item.priority !== "deep-dive").length, 1)) *
      100
  );
  const migrationPulseCategories = useMemo(
    () => ["baseline", "daily", "support", "bridge", "native", "target"],
    []
  );
  const migrationPulseValues = useMemo(
    () => [
      Math.max(0, dailyCount - 1),
      dailyCount,
      dailyCount + 1,
      dailyCount + supportCount - 1,
      dailyCount + supportCount + hybridCount,
      nativeCount,
    ],
    [dailyCount, hybridCount, nativeCount, supportCount]
  );

  function openEntry(entry: DashboardEntry) {
    if (entry.kind === "native" && entry.path) {
      navigate(entry.path);
      return;
    }
    if (entry.kind === "grafana" && entry.uid && grafanaRoot) {
      window.open(grafanaDashboardUrl(grafanaRoot, entry.uid, timeRange, false), "_blank", "noopener,noreferrer");
    }
  }

  return (
    <div className="page-grid dashboard-page">
      {err && <p className="error">{err}</p>}

      <section className="card hero-card">
        <div className="dashboard-hero">
          <div>
            <h2>Analytics command center</h2>
            <p className="meta">
              Daily analyst workspaces stay native first. Grafana is still present, but now positioned as a deep-dive layer rather than the default shell.
            </p>
          </div>
          <div className="btn-row">
            <button type="button" className="secondary" onClick={() => openEntry(current)}>
              Open selected workspace
            </button>
            <a className="tool-btn secondary" href={config?.links.grafana || "#"} target="_blank" rel="noreferrer">
              Open Grafana root
            </a>
          </div>
        </div>
        <div className="summary-grid">
          <div className="summary-card">
            <span>Daily surfaces</span>
            <strong>{dailyCount}</strong>
          </div>
          <div className="summary-card">
            <span>Support surfaces</span>
            <strong>{supportCount}</strong>
          </div>
          <div className="summary-card">
            <span>Native workspaces</span>
            <strong>{nativeCount}</strong>
          </div>
          <div className="summary-card">
            <span>Hybrid bridges</span>
            <strong>{hybridCount}</strong>
          </div>
          <div className="summary-card">
            <span>Deep dives</span>
            <strong>{deepDiveCount || grafanaCount}</strong>
          </div>
        </div>
      </section>

      <AdaptivePaneLayout
        storageKey="dashboards-command-center"
        defaultSizes={[0.34, 0.66]}
        minSizes={[0.26, 0.42]}
        className="analytics-hub-layout"
      >
        <section className="card workspace-pane">
          <div className="workspace-pane-header">
            <div className="workspace-pane-copy">
              <span className="workspace-pane-kicker">Catalog pane</span>
              <h2>Surface catalog</h2>
              <p className="workspace-pane-subtitle">Choose the right surface for daily workflows first, then escalate into bridges and engineering deep-dives.</p>
            </div>
          </div>
          <div className="dashboard-toolbar">
            <div className="dashboard-tabs">
              {DASHBOARD_GROUPS.map((tab) => (
                <button
                  key={tab}
                  type="button"
                  className={tab === group ? "tab-btn active" : "tab-btn secondary"}
                  onClick={() => setGroup(tab)}
                >
                  {tab}
                </button>
              ))}
            </div>
            <label>
              Time range
              <select value={timeRange} onChange={(e) => setTimeRange(e.target.value)}>
                {DASHBOARD_TIME_RANGES.map((range) => (
                  <option key={range.value} value={range.value}>
                    {range.label}
                  </option>
                ))}
              </select>
            </label>
          </div>

          {!!primaryItems.length && (
            <>
              <div className="workspace-pane-header">
                <div className="workspace-pane-copy">
                  <span className="workspace-pane-kicker">Primary surfaces</span>
                  <h2>Daily and support workspaces</h2>
                  <p className="workspace-pane-subtitle">Native operator surfaces intended for the default daily analyst and platform loop.</p>
                </div>
              </div>
              <div className="dashboard-catalog">
                {primaryItems.map((item) => (
                  <button
                    key={item.id}
                    type="button"
                    className={[
                      "dashboard-card",
                      item.id === current.id ? "active" : "",
                      `dashboard-priority-${item.priority}`,
                    ]
                      .filter(Boolean)
                      .join(" ")}
                    onClick={() => setSelectedId(item.id)}
                  >
                    <div className="dashboard-card-head">
                      <strong>{item.title}</strong>
                      <span className={`dashboard-status dashboard-status-${item.status}`}>{item.spotlight ?? item.status}</span>
                    </div>
                    <span>{item.description}</span>
                    <div className="dashboard-chip-row">
                      <span className="token">{item.group}</span>
                      <span className="token">{item.badge}</span>
                      <span className="token">audience: {item.audience}</span>
                    </div>
                  </button>
                ))}
              </div>
            </>
          )}

          {!!secondaryItems.length && (
            <>
              <div className="section-divider" />
              <div className="workspace-pane-header">
                <div className="workspace-pane-copy">
                  <span className="workspace-pane-kicker">Secondary surfaces</span>
                  <h2>Bridges and deep dives</h2>
                  <p className="workspace-pane-subtitle">Use these when the native workspace is not enough and you need raw engineering depth.</p>
                </div>
              </div>
              <div className="dashboard-catalog dashboard-catalog-secondary">
                {secondaryItems.map((item) => (
                  <button
                    key={item.id}
                    type="button"
                    className={[
                      "dashboard-card",
                      "dashboard-card-secondary",
                      item.id === current.id ? "active" : "",
                      `dashboard-priority-${item.priority}`,
                    ]
                      .filter(Boolean)
                      .join(" ")}
                    onClick={() => setSelectedId(item.id)}
                  >
                    <div className="dashboard-card-head">
                      <strong>{item.title}</strong>
                      <span className={`dashboard-status dashboard-status-${item.status}`}>{item.spotlight ?? item.status}</span>
                    </div>
                    <span>{item.description}</span>
                    <div className="dashboard-chip-row">
                      <span className="token">{item.badge}</span>
                      <span className="token">{item.priority}</span>
                      <span className="token">{item.audience}</span>
                    </div>
                  </button>
                ))}
              </div>
            </>
          )}
        </section>

        <section className="card dashboard-shell workspace-pane">
          <div className="dashboard-frame-header">
            <div className="workspace-pane-copy">
              <span className="workspace-pane-kicker">Preview pane</span>
              <h2>{current.title}</h2>
              <p className="meta">{current.description}</p>
            </div>
            <div className="btn-row tight">
              {current.kind === "native" && current.path ? (
                <button type="button" className="secondary" onClick={() => navigate(current.path!)}>
                  Open native workspace
                </button>
              ) : null}
              {current.kind === "grafana" ? (
                <a className="tool-btn inline secondary" href={openUrl || "#"} target="_blank" rel="noreferrer">
                  Pop out in Grafana
                </a>
              ) : null}
            </div>
          </div>

          <div className="summary-grid dashboard-preview-summary">
            <div className="summary-card">
              <span>Surface type</span>
              <strong>{current.kind === "native" ? "Native workspace" : "Grafana deep-dive"}</strong>
            </div>
            <div className="summary-card">
              <span>Priority</span>
              <strong>{current.priority}</strong>
            </div>
            <div className="summary-card">
              <span>Audience</span>
              <strong>{current.audience}</strong>
            </div>
            <div className="summary-card">
              <span>Mode</span>
              <strong>{current.badge}</strong>
            </div>
          </div>

          <div className="dashboard-gauge-grid">
            <ObservabilityGaugePanel
              title="Native migration"
              value={migrationCoverage}
              subtitle="All dashboard surfaces"
              formatter={(value) => `${Math.round(value)}%`}
              kicker="Migration gauge"
              footer={<p className="meta stat-subtle">Shows how much of the catalog already runs natively inside the suite shell.</p>}
            />
            <ObservabilityGaugePanel
              title="Daily native coverage"
              value={dailyNativeCoverage}
              subtitle="Daily and support surfaces"
              formatter={(value) => `${Math.round(value)}%`}
              kicker="Migration gauge"
              footer={<p className="meta stat-subtle">Tracks how much of the default analyst loop has already escaped iframe-first workflows.</p>}
            />
            <ObservabilityLinePanel
              title="Migration pulse"
              subtitle={`${formatCompact(nativeCount)} native surfaces currently available`}
              categories={migrationPulseCategories}
              series={[
                {
                  name: "native surface count",
                  color: "#4d9bff",
                  data: migrationPulseValues,
                  areaOpacity: 0.16,
                },
              ]}
              axisFormatter={(value) => formatCompact(value)}
              valueFormatter={(value) => formatCompact(value)}
              kicker="Migration pane"
              height={270}
              footer={
                <p className="meta stat-subtle">
                  The hub now treats Grafana as a fallback tier while native panels absorb the daily operational language.
                </p>
              }
            />
          </div>

          {current.kind === "native" && current.path ? (
            <div className="dashboard-native-shell">
              <div className="dashboard-native-overview">
                <span className="dashboard-inline-badge">Native surface</span>
                <h3>{current.title} runs directly inside the analyst console</h3>
                <p className="meta">
                  Keep the daily workflow here for speed and shell consistency. Escalate to Grafana only when plugin-based analysis or raw observability depth is required.
                </p>
                <div className="btn-row">
                  <button type="button" onClick={() => navigate(current.path!)}>
                    Go to workspace
                  </button>
                  <Link className="tool-btn secondary" to={current.path}>
                    Open in current shell
                  </Link>
                  {current.id === "operations" ? (
                    <Link className="tool-btn secondary" to="/data-quality">
                      Pair with data quality
                    </Link>
                  ) : null}
                  {current.id === "data-quality" ? (
                    <Link className="tool-btn secondary" to="/operations">
                      Pair with operations
                    </Link>
                  ) : null}
                </div>
              </div>
              <div className="dashboard-native-list">
                <div className="summary-card">
                  <span>Route</span>
                  <strong>{current.path}</strong>
                </div>
                <div className="summary-card">
                  <span>Surface role</span>
                  <strong>{current.priority}</strong>
                </div>
                <div className="summary-card">
                  <span>Escalation path</span>
                  <strong>Grafana available</strong>
                </div>
              </div>
            </div>
          ) : !grafanaRoot ? (
            <p className="error">`links.grafana` did not arrive from `GET /api/v1/ui/config`, so the Grafana deep-dive panel cannot be built.</p>
          ) : (
            <>
              <p className="meta dashboard-note">
                Grafana stays here as a deep-dive layer. If the iframe is empty, verify `GF_SECURITY_ALLOW_EMBEDDING`,
                authentication, and the docker-compose settings for embedded access.
              </p>
              <iframe
                key={`${current.id}-${timeRange}`}
                className="dashboard-frame"
                title={`Grafana dashboard ${current.title}`}
                src={embedUrl}
              />
            </>
          )}
        </section>
      </AdaptivePaneLayout>
    </div>
  );
}
