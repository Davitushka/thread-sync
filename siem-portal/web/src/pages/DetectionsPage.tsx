import { useCallback, useEffect, useMemo, useState } from "react";
import { Link, useNavigate } from "react-router-dom";
import { getDetectionsOverview, type DetectionsOverview } from "../api";
import AdaptivePaneLayout from "../components/AdaptivePaneLayout";
import { NativeBarChart } from "../components/NativeCharts";
import { usePublishPageCommands, type SuitePageCommand } from "../components/SuiteCommandContext";
import { formatCompact } from "../dashboard-utils";

function severityTone(value?: string) {
  return (value || "info").toLowerCase();
}

function priorityFromSeverity(value?: string) {
  const severity = severityTone(value);
  if (severity === "critical") return { label: "P1", tone: "critical" as const };
  if (severity === "error" || severity === "high") return { label: "P2", tone: "high" as const };
  if (severity === "warning") return { label: "P3", tone: "medium" as const };
  return { label: "P4", tone: "low" as const };
}

function stateTone(value?: string) {
  return `state-${(value || "unknown").toLowerCase()}`;
}

export default function DetectionsPage() {
  const navigate = useNavigate();
  const [data, setData] = useState<DetectionsOverview | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [selectedRuleId, setSelectedRuleId] = useState<string | null>(null);
  const [severityFilter, setSeverityFilter] = useState("");
  const [stateFilter, setStateFilter] = useState("");
  const [q, setQ] = useState("");
  const [loading, setLoading] = useState(false);

  const load = useCallback(() => {
    setLoading(true);
    getDetectionsOverview()
      .then((payload) => {
        setData(payload);
        setSelectedRuleId((current) => current ?? payload.rules[0]?.id ?? null);
        setErr(null);
      })
      .catch((e) => setErr(String(e)))
      .finally(() => setLoading(false));
  }, []);

  useEffect(() => {
    load();
  }, [load]);

  const filteredRows = useMemo(() => {
    const rows = data?.firing_rows ?? [];
    return rows.filter((row) => {
      if (severityFilter && row.severity.toLowerCase() !== severityFilter) return false;
      if (stateFilter && row.state.toLowerCase() !== stateFilter) return false;
      if (q.trim()) {
        const needle = q.trim().toLowerCase();
        if (![row.rule, row.signal, row.state, row.severity].join(" ").toLowerCase().includes(needle)) return false;
      }
      return true;
    });
  }, [data, severityFilter, stateFilter, q]);

  const selectedRule = useMemo(() => {
    return data?.rules.find((rule) => rule.id === selectedRuleId) ?? data?.rules[0] ?? null;
  }, [data, selectedRuleId]);

  const catalogRules = useMemo(() => {
    const rules = [...(data?.rules ?? [])];
    const needle = q.trim().toLowerCase();
    return rules
      .filter((rule) => {
        if (severityFilter && severityTone(rule.severity) !== severityFilter) return false;
        if (needle) {
          const haystack = [rule.id, rule.title, rule.kind || "", rule.severity].join(" ").toLowerCase();
          if (!haystack.includes(needle)) return false;
        }
        return true;
      })
      .sort((a, b) => b.firing_count - a.firing_count || a.title.localeCompare(b.title));
  }, [data, severityFilter, q]);

  useEffect(() => {
    if (!catalogRules.length) return;
    if (!catalogRules.some((rule) => rule.id === selectedRuleId)) {
      setSelectedRuleId(catalogRules[0]?.id ?? null);
    }
  }, [catalogRules, selectedRuleId]);

  const matchingRowsForRule = useMemo(() => {
    if (!selectedRule) return [];
    return filteredRows.filter((row) => row.rule === selectedRule.title || row.rule === selectedRule.id);
  }, [filteredRows, selectedRule]);

  const selectedRulePriority = selectedRule ? priorityFromSeverity(selectedRule.severity) : null;
  const selectedRuleGuidance = selectedRule
    ? selectedRulePriority?.tone === "critical"
      ? "Immediate analyst validation recommended: this rule is producing critical pressure and should be correlated with alerts and open cases."
      : selectedRulePriority?.tone === "high"
        ? "High-priority rule pressure: confirm whether the current signal burst should escalate into alert triage or case assignment."
        : "Monitor the firing pattern and use event pivots to validate whether the rule is noisy or expected for the current environment."
    : "";

  const pageCommands = useMemo<SuitePageCommand[]>(() => {
    const commands: SuitePageCommand[] = [
      {
        id: "detections:refresh",
        title: "Refresh detection engine view",
        subtitle: "Reload firing rows, noisy rules and catalog state from the native detections API.",
        section: "Current detection view",
        keywords: "detections refresh reload rules",
        priority: 80,
        run: load,
      },
    ];

    if (severityFilter || stateFilter || q.trim()) {
      commands.push({
        id: "detections:clear-filters",
        title: "Clear detection filters",
        subtitle: "Reset severity, state and free-text filters to restore the full rule set.",
        section: "Current detection view",
        keywords: "detections clear filters reset",
        priority: 85,
        run: () => {
          setSeverityFilter("");
          setStateFilter("");
          setQ("");
        },
      });
    }

    if (selectedRule) {
      commands.push(
        {
          id: `detections:copy:${selectedRule.id}`,
          title: `Copy rule id ${selectedRule.id}`,
          subtitle: "Copy the selected correlator rule identifier to the clipboard.",
          section: "Selected rule",
          keywords: `${selectedRule.id} copy rule`,
          priority: 70,
          run: () => navigator.clipboard.writeText(selectedRule.id),
        },
        {
          id: `detections:events:${selectedRule.id}`,
          title: `Search events for ${selectedRule.title}`,
          subtitle: "Pivot into native event search using the selected rule title.",
          section: "Selected rule",
          keywords: `${selectedRule.title} ${selectedRule.id} events`,
          priority: 100,
          run: () => navigate(`/events?q=${encodeURIComponent(selectedRule.title)}`),
        },
        {
          id: `detections:alerts:${selectedRule.id}`,
          title: "Open alert inbox for follow-up",
          subtitle: "Move from the selected rule into the alert triage queue.",
          section: "Selected rule",
          keywords: `${selectedRule.title} alerts triage`,
          priority: 85,
          run: () => navigate("/alerts"),
        },
        {
          id: `detections:cases:${selectedRule.id}`,
          title: "Search cases for the selected rule",
          subtitle: "Pivot into case operations using the selected rule title as the queue search query.",
          section: "Selected rule",
          keywords: `${selectedRule.title} cases search`,
          priority: 82,
          run: () => navigate(`/cases?q=${encodeURIComponent(selectedRule.title)}`),
        }
      );
    }

    if (matchingRowsForRule[0]?.signal) {
      commands.push({
        id: `detections:signal:${selectedRule?.id ?? "current"}`,
        title: `Search events for signal ${matchingRowsForRule[0].signal}`,
        subtitle: "Use the first matching firing signal as a quick event search pivot.",
        section: "Selected rule",
        keywords: `${matchingRowsForRule[0].signal} signal events`,
        priority: 88,
        run: () => navigate(`/events?q=${encodeURIComponent(matchingRowsForRule[0].signal)}`),
      });
    }

    return commands;
  }, [load, severityFilter, stateFilter, q, selectedRule, matchingRowsForRule, navigate]);

  usePublishPageCommands(pageCommands);

  return (
    <div className="page-grid triage-page">
      {err && <p className="error">{err}</p>}

      <section className="card hero-card triage-card">
        <div className="dashboard-hero">
          <div>
            <h2>Detection engine ops</h2>
            <p className="meta">
              Нативный engine-focused экран: firing rows, noisy rules, correlator catalog и pivots для triage.
            </p>
          </div>
          <div className="dense-inline-actions">
            <button type="button" className="secondary" onClick={load}>
              {loading ? "Refreshing..." : "Refresh"}
            </button>
            <Link className="tool-btn secondary" to="/alerts">
              Open alerts
            </Link>
            <Link className="tool-btn secondary" to="/events">
              Open events
            </Link>
          </div>
        </div>

        <div className="triage-kpi-grid">
          <div className="triage-kpi">
            <span>Rules</span>
            <strong>{formatCompact(data?.stats.rules_count)}</strong>
          </div>
          <div className="triage-kpi">
            <span>Pending alerts</span>
            <strong>{formatCompact(data?.stats.pending_alerts)}</strong>
          </div>
          <div className="triage-kpi">
            <span>Forward queue</span>
            <strong>{formatCompact(data?.stats.alert_capacity)}</strong>
          </div>
          <div className="triage-kpi">
            <span>Firing rows</span>
            <strong>{formatCompact(data?.stats.firing_count)}</strong>
          </div>
          <div className="triage-kpi">
            <span>Critical firing</span>
            <strong>{formatCompact(data?.stats.critical_firing)}</strong>
          </div>
        </div>

        <div className="triage-filterbar">
          <label>
            Severity
            <select value={severityFilter} onChange={(e) => setSeverityFilter(e.target.value)}>
              <option value="">All</option>
              <option value="critical">critical</option>
              <option value="error">error</option>
              <option value="warning">warning</option>
              <option value="info">info</option>
            </select>
          </label>
          <label>
            State
            <select value={stateFilter} onChange={(e) => setStateFilter(e.target.value)}>
              <option value="">All</option>
              <option value="firing">firing</option>
              <option value="pending">pending</option>
              <option value="inactive">inactive</option>
            </select>
          </label>
          <label>
            Search
            <input value={q} onChange={(e) => setQ(e.target.value)} placeholder="rule / state / signal" />
          </label>
        </div>
      </section>

      <AdaptivePaneLayout
        storageKey="detections-command-center"
        defaultSizes={[0.24, 0.46, 0.3]}
        minSizes={[0.18, 0.28, 0.22]}
        className="command-center-layout"
      >
        <section className="card triage-card workspace-pane">
          <div className="workspace-pane-header">
            <div className="workspace-pane-copy">
              <span className="workspace-pane-kicker">Telemetry pane</span>
              <h2>Engine pressure</h2>
              <p className="workspace-pane-subtitle">Severity, state distribution and noisy rule concentration for fast detection posture checks.</p>
            </div>
          </div>
          <div className="section-stack">
            {!data?.severity_breakdown.length ? (
              <p className="meta">Нет severity breakdown.</p>
            ) : (
              <NativeBarChart
                title="Detection severity mix"
                rows={data.severity_breakdown.map((row) => ({
                  label: row.name,
                  value: row.count,
                  tone:
                    row.name === "critical"
                      ? "#f85149"
                      : row.name === "error"
                        ? "#f0883e"
                        : row.name === "warning"
                          ? "#d29922"
                          : "#3fb950",
                }))}
                valueFormatter={(value) => formatCompact(value)}
              />
            )}

            {!data?.state_breakdown.length ? (
              <p className="meta">Нет state breakdown.</p>
            ) : (
              <NativeBarChart
                title="Detection state pressure"
                rows={data.state_breakdown.map((row) => ({
                  label: row.name,
                  value: row.count,
                  tone: row.name === "firing" ? "#f85149" : row.name === "pending" ? "#d29922" : "#4d9bff",
                }))}
                valueFormatter={(value) => formatCompact(value)}
              />
            )}

            {!data?.top_rules.length ? (
              <p className="meta">Нет noisy rules для текущего окна.</p>
            ) : (
              <NativeBarChart
                title="Top noisy rules"
                rows={data.top_rules.map((row) => ({ label: row.name, value: row.count }))}
                valueFormatter={(value) => formatCompact(value)}
              />
            )}
          </div>
        </section>

        <section className="card triage-card workspace-pane">
          <div className="workspace-pane-header">
            <div className="workspace-pane-copy">
              <span className="workspace-pane-kicker">Queue pane</span>
              <h2>Firing queue</h2>
              <p className="workspace-pane-subtitle">Showing {filteredRows.length} active detection rows after the current filters.</p>
            </div>
          </div>
          {!filteredRows.length ? (
            <p className="meta">Нет detection rows под выбранные фильтры.</p>
          ) : (
            <div className="enterprise-table-shell">
              <table className="compact-table enterprise-table">
                <thead>
                  <tr>
                    <th>Priority</th>
                    <th>Rule</th>
                    <th>State</th>
                    <th>Signal</th>
                  </tr>
                </thead>
                <tbody>
                  {filteredRows.map((row, idx) => {
                    const linked = data?.rules.find((rule) => rule.title === row.rule || rule.id === row.rule);
                    const priority = priorityFromSeverity(row.severity);
                    const isActive = linked?.id === selectedRule?.id;
                    return (
                      <tr
                        key={`${row.rule}-${idx}`}
                        className={[
                          "enterprise-row",
                          `enterprise-row-${priority.tone}`,
                          isActive ? "active" : "",
                        ]
                          .filter(Boolean)
                          .join(" ")}
                        onClick={() => linked && setSelectedRuleId(linked.id)}
                      >
                        <td>
                          <span className={`priority-pill priority-${priority.tone}`}>{priority.label}</span>
                        </td>
                        <td>
                          <div className="row-title">
                            <strong>{row.rule}</strong>
                            <small>{row.severity} signal</small>
                          </div>
                        </td>
                        <td>
                          <span className={`token state-pill ${stateTone(row.state)}`}>{row.state}</span>
                        </td>
                        <td>
                          <code>{row.signal}</code>
                        </td>
                      </tr>
                    );
                  })}
                </tbody>
              </table>
            </div>
          )}

          <div className="section-divider" />

          <div className="workspace-pane-header">
            <div className="workspace-pane-copy">
              <span className="workspace-pane-kicker">Catalog pane</span>
              <h2>Rule catalog</h2>
              <p className="workspace-pane-subtitle">Sorted by current firing pressure so the hottest rules stay closest to the analyst.</p>
            </div>
          </div>
          {!catalogRules.length ? (
            <p className="meta">Rules endpoint пуст или текущие фильтры скрыли весь каталог.</p>
          ) : (
            <div className="queue-list">
              {catalogRules.slice(0, 12).map((rule) => {
                const priority = priorityFromSeverity(rule.severity);
                return (
                  <button
                    type="button"
                    key={rule.id}
                    className={[
                      "queue-item",
                      "queue-item-enterprise",
                      `severity-${priority.tone}`,
                      selectedRule?.id === rule.id ? "active" : "",
                    ]
                      .filter(Boolean)
                      .join(" ")}
                    onClick={() => setSelectedRuleId(rule.id)}
                  >
                    <header>
                      <div>
                        <h4>{rule.title || rule.id}</h4>
                        <p className="meta">
                          {rule.kind || "rule"}
                          {rule.threshold ? ` · threshold ${rule.threshold}` : ""}
                        </p>
                      </div>
                      <div className="queue-item-badges">
                        <span className={`priority-pill priority-${priority.tone}`}>{priority.label}</span>
                        <span className={`badge sev-${severityTone(rule.severity)}`}>{rule.severity}</span>
                      </div>
                    </header>
                    <div className="queue-item-meta">
                      <span className="token">firing {formatCompact(rule.firing_count)}</span>
                      {rule.window_sec ? <span className="token">{rule.window_sec}s window</span> : null}
                      <span className="token">{rule.id}</span>
                    </div>
                  </button>
                );
              })}
            </div>
          )}
        </section>

        <aside className="detail-panel">
          <section className="card triage-card detail-section workspace-pane">
            <div className="workspace-pane-header">
              <div className="workspace-pane-copy">
                <span className="workspace-pane-kicker">Detail pane</span>
                <h2>Selected rule</h2>
                <p className="workspace-pane-subtitle">Rule posture, immediate pivots and matching firing context for analyst follow-up.</p>
              </div>
            </div>
            {!selectedRule ? (
              <p className="meta">Выбери rule из catalog или firing queue.</p>
            ) : (
              <>
                <div className="dashboard-hero">
                  <div>
                    <strong>{selectedRule.title}</strong>
                    <p className="meta">{selectedRule.kind || "correlator rule"} · id: {selectedRule.id}</p>
                  </div>
                  <div className="queue-item-badges">
                    {selectedRulePriority ? (
                      <span className={`priority-pill priority-${selectedRulePriority.tone}`}>{selectedRulePriority.label}</span>
                    ) : null}
                    <span className={`badge sev-${severityTone(selectedRule.severity)}`}>{selectedRule.severity}</span>
                  </div>
                </div>

                <div className="detail-metrics">
                  <div className="detail-metric">
                    <span>Firing rows</span>
                    <strong>{formatCompact(selectedRule.firing_count)}</strong>
                  </div>
                  <div className="detail-metric">
                    <span>Threshold</span>
                    <strong>{selectedRule.threshold ?? "—"}</strong>
                  </div>
                  <div className="detail-metric">
                    <span>Window</span>
                    <strong>{selectedRule.window_sec ? `${selectedRule.window_sec}s` : "—"}</strong>
                  </div>
                  <div className="detail-metric">
                    <span>Severity</span>
                    <strong>{selectedRule.severity}</strong>
                  </div>
                </div>

                <div className="insight-panel">
                  <strong>Operator guidance</strong>
                  <p>{selectedRuleGuidance}</p>
                </div>

                <div className="dense-inline-actions">
                  <Link className="tool-btn secondary inline" to="/alerts">
                    Check alert queue
                  </Link>
                  <Link className="tool-btn secondary inline" to={`/events?q=${encodeURIComponent(selectedRule.title)}`}>
                    Pivot to events
                  </Link>
                  <Link className="tool-btn secondary inline" to={`/cases?q=${encodeURIComponent(selectedRule.title)}`}>
                    Pivot to cases
                  </Link>
                </div>

                <div className="section-divider" />

                <div>
                  <p className="meta">Matching firing rows</p>
                  {!matchingRowsForRule.length ? (
                    <p className="meta">Нет firing rows для выбранного правила.</p>
                  ) : (
                    <div className="queue-list queue-list-dense">
                      {matchingRowsForRule.map((row, idx) => {
                        const priority = priorityFromSeverity(row.severity);
                        return (
                          <div key={`${row.rule}-${idx}`} className={`queue-item queue-item-enterprise severity-${priority.tone}`}>
                            <header>
                              <div>
                                <h4>{row.rule}</h4>
                                <p className="meta">Signal {row.signal}</p>
                              </div>
                              <div className="queue-item-badges">
                                <span className={`priority-pill priority-${priority.tone}`}>{priority.label}</span>
                                <span className={`badge sev-${severityTone(row.severity)}`}>{row.severity}</span>
                              </div>
                            </header>
                            <div className="queue-item-meta">
                              <span className={`token state-pill ${stateTone(row.state)}`}>{row.state}</span>
                              <span className="token">{row.signal}</span>
                            </div>
                          </div>
                        );
                      })}
                    </div>
                  )}
                </div>
              </>
            )}
          </section>
        </aside>
      </AdaptivePaneLayout>
    </div>
  );
}
