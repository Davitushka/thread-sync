import { useCallback, useEffect, useMemo, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { createCase, getAlertsOverview, linkAlert, type AlertsOverview } from "../api";
import { ObservabilityBarPanel, ObservabilityGaugePanel, ObservabilityLinePanel } from "../components/echarts/ObservabilityCharts";
import { useActorState } from "../components/PageLayout";
import { usePublishPageCommands, type SuitePageCommand } from "../components/SuiteCommandContext";
import { formatCompact, shortDateTime } from "../dashboard-utils";

function severity(value?: string) {
  return (value || "unknown").toLowerCase();
}

export default function AlertsPage() {
  const navigate = useNavigate();
  const { actor, setActor } = useActorState();
  const [data, setData] = useState<AlertsOverview | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [creating, setCreating] = useState<string | null>(null);
  const [selected, setSelected] = useState<string | null>(null);
  const [severityFilter, setSeverityFilter] = useState("");
  const [stateFilter, setStateFilter] = useState("");
  const [sourceFilter, setSourceFilter] = useState("");
  const [q, setQ] = useState("");

  const load = useCallback(
    () =>
      getAlertsOverview()
        .then((payload) => {
          setData(payload);
          setSelected((current) => current ?? payload.alerts[0]?.fingerprint ?? null);
        })
        .catch((e) => setErr(String(e))),
    []
  );

  useEffect(() => {
    load();
  }, [load]);

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

  const totalAlerts = data?.totals.total ?? 0;
  const activeShare = totalAlerts ? ((data?.totals.active ?? 0) / totalAlerts) * 100 : 0;
  const criticalShare = totalAlerts ? ((data?.totals.critical ?? 0) / totalAlerts) * 100 : 0;
  const silencedShare = totalAlerts ? ((data?.totals.silenced ?? 0) / totalAlerts) * 100 : 0;
  const sourceSpread = totalAlerts ? ((data?.totals.unique_sources ?? 0) / totalAlerts) * 100 : 0;
  const alertTimeline = useMemo(() => {
    const buckets = new Map<string, { total: number; critical: number; active: number }>();
    for (const alert of filteredAlerts) {
      const iso = alert.starts_at || alert.ends_at;
      const parsed = iso ? new Date(iso) : null;
      const label =
        parsed && !Number.isNaN(parsed.getTime())
          ? parsed.toLocaleTimeString([], { hour: "2-digit", minute: "2-digit" })
          : "unknown";
      const entry = buckets.get(label) ?? { total: 0, critical: 0, active: 0 };
      entry.total += 1;
      if (severity(alert.severity) === "critical") entry.critical += 1;
      if (!(alert.silenced_count > 0 || alert.state === "suppressed")) entry.active += 1;
      buckets.set(label, entry);
    }
    return Array.from(buckets.entries())
      .sort((a, b) => a[0].localeCompare(b[0]))
      .slice(-12);
  }, [filteredAlerts]);

  const promote = useCallback(async () => {
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
  }, [actor, load, selectedAlert]);

  const pageCommands = useMemo<SuitePageCommand[]>(() => {
    const commands: SuitePageCommand[] = [
      {
        id: "alerts:refresh",
        title: "Refresh alert inbox",
        subtitle: "Reload the native alert queue and keep the current selection when possible.",
        section: "Current alert inbox",
        keywords: "alerts refresh reload",
        priority: 80,
        run: load,
      },
    ];

    if (severityFilter || stateFilter || sourceFilter || q.trim()) {
      commands.push({
        id: "alerts:clear-filters",
        title: "Clear alert filters",
        subtitle: "Reset severity, state, source and search filters back to the full inbox.",
        section: "Current alert inbox",
        keywords: "alerts clear filters reset",
        priority: 85,
        run: () => {
          setSeverityFilter("");
          setStateFilter("");
          setSourceFilter("");
          setQ("");
        },
      });
    }

    if (selectedAlert) {
      commands.push(
        {
          id: `alerts:promote:${selectedAlert.fingerprint}`,
          title: `Promote ${selectedAlert.name} to case`,
          subtitle: "Create a case from the selected alert and link the alert artifact automatically.",
          section: "Selected alert",
          keywords: `${selectedAlert.name} promote case ${selectedAlert.rule_id ?? ""}`,
          priority: 100,
          run: promote,
        },
        {
          id: `alerts:copy:${selectedAlert.fingerprint}`,
          title: "Copy selected alert fingerprint",
          subtitle: "Copy the selected alert fingerprint to the clipboard for pivots or sharing.",
          section: "Selected alert",
          keywords: `${selectedAlert.fingerprint} copy fingerprint`,
          priority: 75,
          run: () => navigator.clipboard.writeText(selectedAlert.fingerprint),
        }
      );

      if (selectedAlert.source_ip) {
        commands.push({
          id: `alerts:ip:${selectedAlert.fingerprint}`,
          title: `Search events for ${selectedAlert.source_ip}`,
          subtitle: "Pivot into native event search using the selected alert source IP.",
          section: "Selected alert",
          keywords: `${selectedAlert.source_ip} events ip alert`,
          priority: 95,
          run: () => navigate(`/events?source_ip=${encodeURIComponent(selectedAlert.source_ip || "")}`),
        });
      }
      if (selectedAlert.rule_id) {
        commands.push({
          id: `alerts:rule:${selectedAlert.fingerprint}`,
          title: `Search events for rule ${selectedAlert.rule_id}`,
          subtitle: "Pivot into native event search using the selected alert rule identifier.",
          section: "Selected alert",
          keywords: `${selectedAlert.rule_id} rule events alert`,
          priority: 90,
          run: () => navigate(`/events?q=${encodeURIComponent(selectedAlert.rule_id || "")}`),
        });
      }
      if (selectedAlert.user_id) {
        commands.push({
          id: `alerts:user:${selectedAlert.fingerprint}`,
          title: `Search events for user ${selectedAlert.user_id}`,
          subtitle: "Pivot into native event search using the selected alert user identifier.",
          section: "Selected alert",
          keywords: `${selectedAlert.user_id} user events alert`,
          priority: 90,
          run: () => navigate(`/events?user_id=${encodeURIComponent(selectedAlert.user_id || "")}`),
        });
      }
    }

    return commands;
  }, [load, severityFilter, stateFilter, sourceFilter, q, selectedAlert, promote, navigate]);

  usePublishPageCommands(pageCommands);

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

      <section className="dashboard-gauge-grid">
        <ObservabilityGaugePanel
          title="Active share"
          subtitle="Open triage load"
          value={activeShare}
          formatter={(value) => `${value.toFixed(1)}%`}
          kicker="Queue gauge"
          footer={<p className="meta stat-subtle">{formatCompact(data?.totals.active)} active alerts in the current inbox.</p>}
        />
        <ObservabilityGaugePanel
          title="Critical share"
          subtitle="Highest risk slice"
          value={criticalShare}
          formatter={(value) => `${value.toFixed(1)}%`}
          kicker="Risk gauge"
          footer={<p className="meta stat-subtle">{formatCompact(data?.totals.critical)} alerts currently sit in the highest-priority bucket.</p>}
        />
        <ObservabilityGaugePanel
          title="Silenced share"
          subtitle="Suppression coverage"
          value={silencedShare}
          formatter={(value) => `${value.toFixed(1)}%`}
          kicker="State gauge"
          footer={<p className="meta stat-subtle">{formatCompact(data?.totals.silenced)} alerts are currently suppressed or silenced.</p>}
        />
        <ObservabilityGaugePanel
          title="Source spread"
          subtitle="Source diversity"
          value={sourceSpread}
          formatter={(value) => `${value.toFixed(1)}%`}
          kicker="Source gauge"
          footer={<p className="meta stat-subtle">{formatCompact(data?.totals.unique_sources)} distinct sources are contributing to the current queue.</p>}
        />
      </section>

      {!!alertTimeline.length && (
        <ObservabilityLinePanel
          title="Alert pressure strip"
          subtitle="Recent alert starts grouped into a compact triage rhythm"
          categories={alertTimeline.map(([label]) => label)}
          series={[
            {
              name: "alerts",
              color: "#4d9bff",
              data: alertTimeline.map(([, value]) => value.total),
              areaOpacity: 0.16,
            },
            {
              name: "critical",
              color: "#f85149",
              data: alertTimeline.map(([, value]) => value.critical),
            },
            {
              name: "active",
              color: "#f0c15d",
              data: alertTimeline.map(([, value]) => value.active),
            },
          ]}
          axisFormatter={(value) => formatCompact(value)}
          valueFormatter={(value) => formatCompact(value)}
          kicker="Pressure pane"
          showDataZoom
          footer={<p className="meta stat-subtle">This is a lightweight pressure strip derived from the current inbox snapshot, useful for quick triage pacing.</p>}
        />
      )}

      <section className="triage-grid">
        <div className="section-stack">
          <ObservabilityBarPanel
            title="Severity mix"
            subtitle="Live severity pressure in the current inbox"
            rows={(data?.severity_breakdown ?? []).map((row) => ({
              label: row.name,
              value: row.count,
              color:
                row.name === "critical"
                  ? "#f85149"
                  : row.name === "high" || row.name === "error"
                    ? "#f0883e"
                    : row.name === "warning"
                      ? "#d29922"
                      : "#3fb950",
            }))}
            valueFormatter={(value) => formatCompact(value)}
            axisFormatter={(value) => formatCompact(value)}
            kicker="Analytics pane"
            onRowClick={({ label }) => setSeverityFilter(label)}
            footer={<p className="meta stat-subtle">This is the quickest read on how much of the queue needs immediate analyst attention.</p>}
          />
          <ObservabilityBarPanel
            title="Top sources"
            subtitle="Biggest source contributors to the queue"
            rows={(data?.source_breakdown ?? []).map((row) => ({ label: row.name, value: row.count, color: "#4d9bff" }))}
            valueFormatter={(value) => formatCompact(value)}
            axisFormatter={(value) => formatCompact(value)}
            kicker="Analytics pane"
            onRowClick={({ label }) => setSourceFilter(label)}
            footer={<p className="meta stat-subtle">Useful for seeing whether one platform or tenant is dominating alert production.</p>}
          />
        </div>

        <article className="card triage-card workspace-pane">
          <div className="workspace-pane-header">
            <div className="workspace-pane-copy">
              <span className="workspace-pane-kicker">Inbox pane</span>
              <h2>Alert queue</h2>
              <p className="workspace-pane-subtitle">Showing {filteredAlerts.length} alert rows after the current filter set.</p>
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
          <section className="card triage-card detail-section workspace-pane">
            <div className="workspace-pane-header">
              <div className="workspace-pane-copy">
                <span className="workspace-pane-kicker">Detail pane</span>
                <h2>Selected alert</h2>
                <p className="workspace-pane-subtitle">Focused alert context, labels and direct promotion or pivot actions.</p>
              </div>
            </div>
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
                    <small>{selectedAlert.fingerprint}</small>
                  </div>
                  <div className="detail-metric">
                    <span>Ended</span>
                    <strong>{selectedAlert.ends_at ? shortDateTime(selectedAlert.ends_at) : "still firing"}</strong>
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
                      .slice(0, 14)
                      .map(([key, value]) => (
                        <span key={key} className="token">
                          {key}:{value}
                        </span>
                      ))}
                  </div>
                </div>

                {!!Object.keys(selectedAlert.annotations).length && (
                  <div>
                    <p className="meta">Annotations</p>
                    <div className="tag-row">
                      {Object.entries(selectedAlert.annotations).map(([key, value]) => (
                        <span key={key} className="token">
                          {key}:{value}
                        </span>
                      ))}
                    </div>
                  </div>
                )}
              </>
            )}
          </section>
        </aside>
      </section>
    </div>
  );
}
