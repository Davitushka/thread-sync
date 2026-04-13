import { useEffect, useMemo, useState } from "react";
import { Link } from "react-router-dom";
import { getDetectionsOverview, type DetectionsOverview } from "../api";
import { NativeBarChart } from "../components/NativeCharts";
import { formatCompact } from "../dashboard-utils";

export default function DetectionsPage() {
  const [data, setData] = useState<DetectionsOverview | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [selectedRuleId, setSelectedRuleId] = useState<string | null>(null);
  const [severityFilter, setSeverityFilter] = useState("");
  const [stateFilter, setStateFilter] = useState("");
  const [q, setQ] = useState("");
  const [loading, setLoading] = useState(false);

  const load = () => {
    setLoading(true);
    getDetectionsOverview()
      .then((payload) => {
        setData(payload);
        setSelectedRuleId((current) => current ?? payload.rules[0]?.id ?? null);
        setErr(null);
      })
      .catch((e) => setErr(String(e)))
      .finally(() => setLoading(false));
  };

  useEffect(() => {
    load();
  }, []);

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

  const matchingRowsForRule = useMemo(() => {
    if (!selectedRule) return [];
    return filteredRows.filter((row) => row.rule === selectedRule.title || row.rule === selectedRule.id);
  }, [filteredRows, selectedRule]);

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

      <section className="triage-grid">
        <article className="card triage-card">
          <h2>Detection mix</h2>
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

          <h2>Top noisy rules</h2>
          {!data?.top_rules.length ? (
            <p className="meta">Нет noisy rules для текущего окна.</p>
          ) : (
            <NativeBarChart
              title="Top noisy rules"
              rows={data.top_rules.map((row) => ({ label: row.name, value: row.count }))}
              valueFormatter={(value) => formatCompact(value)}
            />
          )}
        </article>

        <article className="card triage-card">
          <div className="dashboard-hero">
            <div>
              <h2>Firing queue</h2>
              <p className="meta">Показывает {filteredRows.length} firing rows после фильтров.</p>
            </div>
          </div>
          {!filteredRows.length ? (
            <p className="meta">Нет detection rows под выбранные фильтры.</p>
          ) : (
            <table className="compact-table">
              <thead>
                <tr>
                  <th>Rule</th>
                  <th>Severity</th>
                  <th>State</th>
                  <th>Signal</th>
                </tr>
              </thead>
              <tbody>
                {filteredRows.map((row, idx) => (
                  <tr
                    key={`${row.rule}-${idx}`}
                    onClick={() => {
                      const linked = data?.rules.find((rule) => rule.title === row.rule || rule.id === row.rule);
                      if (linked) setSelectedRuleId(linked.id);
                    }}
                    style={{ cursor: "pointer" }}
                  >
                    <td>{row.rule}</td>
                    <td>
                      <span className={`badge sev-${row.severity.toLowerCase()}`}>{row.severity}</span>
                    </td>
                    <td>{row.state}</td>
                    <td>{row.signal}</td>
                  </tr>
                ))}
              </tbody>
            </table>
          )}

          <h2>Rule catalog</h2>
          {!data?.rules.length ? (
            <p className="meta">Rules endpoint пуст или ещё не подключён.</p>
          ) : (
            <div className="queue-list">
              {data.rules.slice(0, 10).map((rule) => (
                <button
                  type="button"
                  key={rule.id}
                  className={selectedRule?.id === rule.id ? "queue-item active" : "queue-item"}
                  onClick={() => setSelectedRuleId(rule.id)}
                >
                  <header>
                    <div>
                      <h4>{rule.title || rule.id}</h4>
                      <p className="meta">{rule.kind || "rule"} {rule.threshold ? `· threshold ${rule.threshold}` : ""}</p>
                    </div>
                    <span className={`badge sev-${rule.severity.toLowerCase()}`}>{rule.severity}</span>
                  </header>
                  <div className="queue-item-meta">
                    <span className="token">firing {rule.firing_count}</span>
                    {rule.window_sec ? <span className="token">{rule.window_sec}s</span> : null}
                  </div>
                </button>
              ))}
            </div>
          )}
        </article>

        <aside className="detail-panel">
          <section className="card triage-card detail-section">
            <h2>Selected rule</h2>
            {!selectedRule ? (
              <p className="meta">Выбери rule из catalog или firing queue.</p>
            ) : (
              <>
                <div className="dashboard-hero">
                  <div>
                    <strong>{selectedRule.title}</strong>
                    <p className="meta">{selectedRule.kind || "correlator rule"} · id: {selectedRule.id}</p>
                  </div>
                  <span className={`badge sev-${selectedRule.severity.toLowerCase()}`}>{selectedRule.severity}</span>
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

                <div className="dense-inline-actions">
                  <Link className="tool-btn secondary inline" to="/alerts">
                    Check alert queue
                  </Link>
                  <Link className="tool-btn secondary inline" to={`/events?q=${encodeURIComponent(selectedRule.title)}`}>
                    Pivot to events
                  </Link>
                  <Link className="tool-btn secondary inline" to="/cases">
                    Pivot to cases
                  </Link>
                </div>

                <div>
                  <p className="meta">Matching firing rows</p>
                  {!matchingRowsForRule.length ? (
                    <p className="meta">Нет firing rows для выбранного правила.</p>
                  ) : (
                    <div className="queue-list">
                      {matchingRowsForRule.map((row, idx) => (
                        <div key={`${row.rule}-${idx}`} className="queue-item">
                          <header>
                            <div>
                              <h4>{row.rule}</h4>
                              <p className="meta">Signal {row.signal}</p>
                            </div>
                            <span className={`badge sev-${row.severity.toLowerCase()}`}>{row.severity}</span>
                          </header>
                          <div className="queue-item-meta">
                            <span className="token">{row.state}</span>
                            <span className="token">{row.signal}</span>
                          </div>
                        </div>
                      ))}
                    </div>
                  )}
                </div>
              </>
            )}
          </section>
        </aside>
      </section>
    </div>
  );
}
