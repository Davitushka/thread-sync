import { useEffect, useMemo, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { uiConfig, type UiConfig } from "../api";
import {
  DASHBOARDS,
  DASHBOARD_GROUPS,
  DASHBOARD_TIME_RANGES,
  grafanaDashboardUrl,
  type DashboardEntry,
} from "../dashboard-catalog";

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
            <h2>Native dashboards hub</h2>
            <p className="meta">
              Everyday SOC workflows now stay inside native suite screens first. Grafana remains available for advanced
              platform and deep-dive analysis instead of being the default shell for everything.
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
            <span>Native workspaces</span>
            <strong>{nativeCount}</strong>
          </div>
          <div className="summary-card">
            <span>Hybrid bridges</span>
            <strong>{hybridCount}</strong>
          </div>
          <div className="summary-card">
            <span>Grafana deep dives</span>
            <strong>{grafanaCount}</strong>
          </div>
          <div className="summary-card">
            <span>Current mode</span>
            <strong>{current.kind === "native" ? "Native" : "Grafana"}</strong>
          </div>
        </div>
      </section>

      <section className="card">
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

        <div className="dashboard-catalog">
          {items.map((item) => (
            <button
              key={item.id}
              type="button"
              className={item.id === current.id ? "dashboard-card active" : "dashboard-card"}
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
              </div>
            </button>
          ))}
        </div>
      </section>

      <section className="card dashboard-shell">
        <div className="dashboard-frame-header">
          <div>
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

        {current.kind === "native" && current.path ? (
          <div className="dashboard-native-shell">
            <div className="dashboard-native-overview">
              <span className="dashboard-inline-badge">Native workspace</span>
              <h3>{current.title} runs directly inside Unified Suite</h3>
              <p className="meta">
                This workspace is no longer dependent on embedded Grafana. Use it for daily operations, then pivot to
                Grafana only when you need deeper technical charts or plugin-based drill-down.
              </p>
              <div className="btn-row">
                <button type="button" onClick={() => navigate(current.path!)}>
                  Go to workspace
                </button>
                <Link className="tool-btn secondary" to={current.path}>
                  Open in current window
                </Link>
                {current.id === "operations" ? (
                  <Link className="tool-btn secondary" to="/data-quality">
                    Open data quality too
                  </Link>
                ) : null}
                {current.id === "data-quality" ? (
                  <Link className="tool-btn secondary" to="/operations">
                    Open operations too
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
                <span>Mode</span>
                <strong>{current.badge}</strong>
              </div>
              <div className="summary-card">
                <span>Fallback</span>
                <strong>Grafana still available</strong>
              </div>
            </div>
          </div>
        ) : !grafanaRoot ? (
          <p className="error">`links.grafana` did not arrive from `GET /api/v1/ui/config`, so the Grafana deep-dive panel cannot be built.</p>
        ) : (
          <>
            <p className="meta dashboard-note">
              Grafana stays here as a deep-dive layer. If the iframe is empty, verify `GF_SECURITY_ALLOW_EMBEDDING`,
              authentication, and the local docker-compose settings for embedded access.
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
    </div>
  );
}
