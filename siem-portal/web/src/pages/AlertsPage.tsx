import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { createCase, getAlertsOverview, linkAlert, type AlertsOverview } from "../api";
import { NativeBarChart } from "../components/NativeCharts";
import { useActorState } from "../components/PageLayout";
import { formatCompact, shortDateTime } from "../dashboard-utils";

function severity(value?: string) {
  return (value || "unknown").toLowerCase();
}

export default function AlertsPage() {
  const { actor, setActor } = useActorState();
  const [data, setData] = useState<AlertsOverview | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [creating, setCreating] = useState<string | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [severityFilter, setSeverityFilter] = useState("");
  const [stateFilter, setStateFilter] = useState("");
  const [sourceFilter, setSourceFilter] = useState("");
  const [q, setQ] = useState("");

  const load = () =>
    getAlertsOverview()
      .then((payload) => {
        setData(payload);
        setSelected((current) => current ?? payload.alerts[0]?.fingerprint ?? null);
      })
      .catch((e) => setErr(String(e)));

  useEffect(() => {
    load();
  }, []);

  const filteredAlerts = useMemo(() => {
    const rows = data?.alerts ?? [];
    return rows.filter((alert) => {
      if (severityFilter && severity(alert.severity) !== severityFilter) return false;
      if (stateFilter === "active" && (alert.silenced_count > 0 || alert.state === "suppressed")) return false;
      if (stateFilter === "silenced" && !(alert.silenced_count > 0 || alert.state === "suppressed")) return false;
      if (stateFilter && !["active", "silenced"].includes(stateFilter) && alert.state !== stateFilter) return false;
      if (sourceFilter && alert.source !== sourceFilter) return false;
      if (q.trim()) {
        const needle = q.trim().toLowerCase();
        const hay = [alert.name, alert.summary, alert.description, alert.source, alert.rule_id || "", alert.source_ip || ""]
          .join(" ")
          .toLowerCase();
        if (!hay.includes(needle)) return false;
      }
      return true;
    });
  }, [data, severityFilter, stateFilter, sourceFilter, q]);

  const selectedAlert = useMemo(
    () => filteredAlerts.find((alert) => alert.fingerprint === selected) ?? filteredAlerts[0] ?? null,
    [filteredAlerts, selected]
  );

  const sources = useMemo(() => {
    return Array.from(new Set((data?.alerts ?? []).map((alert) => alert.source))).sort((a, b) => a.localeCompare(b));
  }, [data]);

  const promote = async () => {
    if (!selectedAlert) return;
    setCreating(selectedAlert.fingerprint);
    setErr(null);
    try {
      const created = await createCase(
        {
          title: selectedAlert.name || "Alert",
          description: selectedAlert.description || selectedAlert.summary || "Promoted from alert inbox",
          severity: severity(selectedAlert.severity),
        },
        actor
      );
      await linkAlert(
        created.id,
        selectedAlert.fingerprint,
        {
          rule_id: selectedAlert.rule_id,
          rule_title: selectedAlert.name,
          severity: selectedAlert.severity,
          description: selectedAlert.description,
          context: {
            source: selectedAlert.source,
            source_ip: selectedAlert.source_ip,
            user_id: selectedAlert.user_id,
            state: selectedAlert.state,
          },
        },
        actor
      );
      await load();
    } catch (e) {
      setErr(String(e));
    } finally {
      setCreating(null);
    }
  };

  return (
    <div className="page-grid triage-page">
      {err && <p className="error">{err}</p>}

      <section className="card hero-card triage-card">
        <div className="dashboard-hero">
          <div>
            <h2>Dense alert inbox</h2>
            <p className="meta">
              Нативный triage-экран поверх Alertmanager: плотная очередь, detail pane и быстрые переходы в кейсы.
            </p>
          </div>
          <div className="dense-inline-actions">
            <button type="button" className="secondary" onClick={load}>
              Refresh
            </button>
            <Link className="tool-btn secondary" to="/cases">
              Open cases
            </Link>
            <Link className="tool-btn secondary" to="/events">
              Pivot to events
            </Link>
          </div>
        </div>

        <div className="triage-kpi-grid">
          <div className="triage-kpi">
            <span>Total alerts</span>
            <strong>{formatCompact(data?.totals.total)}</strong>
          </div>
          <div className="triage-kpi">
            <span>Active</span>
            <strong>{formatCompact(data?.totals.active)}</strong>
          </div>
          <div className="triage-kpi">
            <span>Critical</span>
            <strong>{formatCompact(data?.totals.critical)}</strong>
          </div>
          <div className="triage-kpi">
            <span>Silenced</span>
            <strong>{formatCompact(data?.totals.silenced)}</strong>
          </div>
          <div className="triage-kpi">
            <span>Unique sources</span>
            <strong>{formatCompact(data?.totals.unique_sources)}</strong>
          </div>
        </div>

        <div className="triage-filterbar">
          <label>
            Analyst
            <input value={actor} onChange={(e) => setActor(e.target.value)} />
          </label>
          <label>
            Severity
            <select value={severityFilter} onChange={(e) => setSeverityFilter(e.target.value)}>
              <option value="">All</option>
              <option value="critical">critical</option>
              <option value="high">high</option>
              <option value="error">error</option>
              <option value="warning">warning</option>
              <option value="info">info</option>
            </select>
          </label>
          <label>
            State
            <select value={stateFilter} onChange={(e) => setStateFilter(e.target.value)}>
              <option value="">All</option>
              <option value="active">active</option>
              <option value="silenced">silenced</option>
              <option value="firing">firing</option>
            </select>
          </label>
          <label>
            Source
            <select value={sourceFilter} onChange={(e) => setSourceFilter(e.target.value)}>
              <option value="">All</option>
              {sources.map((source) => (
                <option key={source} value={source}>
                  {source}
                </option>
              ))}
            </select>
          </label>
          <label>
            Search
            <input value={q} onChange={(e) => setQ(e.target.value)} placeholder="alert name / source / fingerprint" />
          </label>
        </div>
      </section>

      <section className="triage-grid">
        <article className="card triage-card">
          <h2>Severity mix</h2>
          {!data?.severity_breakdown.length ? (
            <p className="meta">Нет данных severity breakdown.</p>
          ) : (
            <NativeBarChart
              title="Alert severity mix"
              rows={data.severity_breakdown.map((row) => ({
                label: row.name,
                value: row.count,
                tone:
                  row.name === "critical"
                    ? "#f85149"
                    : row.name === "high" || row.name === "error"
                      ? "#f0883e"
                      : row.name === "warning"
                        ? "#d29922"
                        : "#3fb950",
              }))}
              valueFormatter={(value) => formatCompact(value)}
            />
          )}
          <h2>Top sources</h2>
          {!data?.source_breakdown.length ? (
            <p className="meta">Нет source breakdown.</p>
          ) : (
            <NativeBarChart
              title="Alert top sources"
              rows={data.source_breakdown.map((row) => ({ label: row.name, value: row.count }))}
              valueFormatter={(value) => formatCompact(value)}
            />
          )}
        </article>

        <article className="card triage-card">
          <div className="dashboard-hero">
            <div>
              <h2>Alert queue</h2>
              <p className="meta">Показывает {filteredAlerts.length} alert rows после фильтрации.</p>
            </div>
          </div>
          {!filteredAlerts.length ? (
            <p className="meta">Нет alert rows под выбранные фильтры.</p>
          ) : (
            <div className="queue-list">
              {filteredAlerts.map((alert) => (
                <button
                  type="button"
                  key={alert.fingerprint}
                  className={alert.fingerprint === selectedAlert?.fingerprint ? "queue-item active" : "queue-item"}
                  onClick={() => setSelected(alert.fingerprint)}
                >
                  <header>
                    <div>
                      <h3>{alert.name}</h3>
                      <p className="meta">{alert.summary}</p>
                    </div>
                    <span className={`badge sev-${severity(alert.severity)}`}>{alert.severity}</span>
                  </header>
                  <div className="queue-item-meta">
                    <span className="token">{alert.source}</span>
                    {alert.rule_id ? <span className="token">{alert.rule_id}</span> : null}
                    <span className="token">{alert.state}</span>
                    {alert.silenced_count > 0 ? <span className="token">silenced x{alert.silenced_count}</span> : null}
                    <span className="token">{shortDateTime(alert.starts_at || "")}</span>
                  </div>
                </button>
              ))}
            </div>
          )}
        </article>

        <aside className="detail-panel">
          <section className="card triage-card detail-section">
            <h2>Selected alert</h2>
            {!selectedAlert ? (
              <p className="meta">Выбери alert row слева.</p>
            ) : (
              <>
                <div className="dashboard-hero">
                  <div>
                    <strong>{selectedAlert.name}</strong>
                    <p className="meta">{selectedAlert.description}</p>
                  </div>
                  <span className={`badge sev-${severity(selectedAlert.severity)}`}>{selectedAlert.severity}</span>
                </div>

                <div className="detail-metrics">
                  <div className="detail-metric">
                    <span>Source</span>
                    <strong>{selectedAlert.source}</strong>
                  </div>
                  <div className="detail-metric">
                    <span>State</span>
                    <strong>{selectedAlert.state}</strong>
                  </div>
                  <div className="detail-metric">
                    <span>Started</span>
                    <strong>{shortDateTime(selectedAlert.starts_at || "")}</strong>
                  </div>
                  <div className="detail-metric">
                    <span>Fingerprint</span>
                    <strong>{selectedAlert.fingerprint.slice(0, 12)}...</strong>
                  </div>
                </div>

                <div className="tag-row">
                  {selectedAlert.source_ip ? <span className="token">ip:{selectedAlert.source_ip}</span> : null}
                  {selectedAlert.user_id ? <span className="token">user:{selectedAlert.user_id}</span> : null}
                  {selectedAlert.rule_id ? <span className="token">rule:{selectedAlert.rule_id}</span> : null}
                  {selectedAlert.silenced_count > 0 ? <span className="token">silenced</span> : <span className="token">active</span>}
                </div>

                <div className="dense-inline-actions">
                  <button type="button" onClick={promote} disabled={creating === selectedAlert.fingerprint}>
                    {creating === selectedAlert.fingerprint ? "Creating..." : "Promote to case"}
                  </button>
                  <button
                    type="button"
                    className="secondary"
                    onClick={() => navigator.clipboard.writeText(selectedAlert.fingerprint)}
                  >
                    Copy fingerprint
                  </button>
                  {selectedAlert.source_ip ? (
                    <Link className="tool-btn secondary inline" to={`/events?source_ip=${encodeURIComponent(selectedAlert.source_ip)}`}>
                      Pivot by IP
                    </Link>
                  ) : null}
                  {selectedAlert.rule_id ? (
                    <Link className="tool-btn secondary inline" to={`/events?q=${encodeURIComponent(selectedAlert.rule_id)}`}>
                      Pivot by rule
                    </Link>
                  ) : null}
                </div>

                <div>
                  <p className="meta">Labels</p>
                  <div className="tag-row">
                    {Object.entries(selectedAlert.labels)
                      .slice(0, 10)
                      .map(([key, value]) => (
                        <span key={key} className="token">
                          {key}:{value}
                        </span>
                      ))}
                  </div>
                </div>
              </>
            )}
          </section>
        </aside>
      </section>
    </div>
  );
}
