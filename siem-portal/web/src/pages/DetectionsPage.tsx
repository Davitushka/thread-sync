import { useEffect, useState } from "react";
import { getCorrelatorRules, getCorrelatorStats, getPromAlerts, type CorrelatorStats, type DetectionRow, type RuleCard } from "../api";

export default function DetectionsPage() {
  const [stats, setStats] = useState<CorrelatorStats | null>(null);
  const [rules, setRules] = useState<RuleCard[]>([]);
  const [detections, setDetections] = useState<DetectionRow[]>([]);
  const [err, setErr] = useState<string | null>(null);

  useEffect(() => {
    Promise.all([getCorrelatorStats(), getCorrelatorRules(), getPromAlerts()])
      .then(([statsData, rulesData, promRows]) => {
        setStats(statsData);
        setRules(rulesData);
        setDetections(promRows);
      })
      .catch((e) => setErr(String(e)));
  }, []);

  return (
    <div className="page-grid">
      {err && <p className="error">{err}</p>}
      <section className="card">
        <h2>Detection engine</h2>
        <div className="kpi-grid">
          <div className="kpi-card">
            <span>Rules</span>
            <strong>{stats?.rules_count ?? "—"}</strong>
          </div>
          <div className="kpi-card">
            <span>Pending alerts</span>
            <strong>{stats?.pending_alerts ?? "—"}</strong>
          </div>
          <div className="kpi-card">
            <span>Forward queue</span>
            <strong>{stats?.alert_capacity ?? "—"}</strong>
          </div>
        </div>
      </section>

      <section className="card">
        <h2>Firing detections</h2>
        {!detections.length ? (
          <p className="meta">Нет активных detection rows.</p>
        ) : (
          <table>
            <thead>
              <tr>
                <th>Rule</th>
                <th>Severity</th>
                <th>State</th>
                <th>Signal</th>
              </tr>
            </thead>
            <tbody>
              {detections.map((row, idx) => (
                <tr key={`${row.rule}-${idx}`}>
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
      </section>

      <section className="card">
        <h2>Correlator rules</h2>
        {!rules.length ? (
          <p className="meta">Rules endpoint пуст или ещё не подключён.</p>
        ) : (
          <div className="home-grid">
            {rules.map((rule) => (
              <article key={rule.id} className="home-card">
                <h2>{rule.title || rule.id}</h2>
                <p>
                  Severity: {rule.severity || "—"}
                  {rule.kind ? ` · ${rule.kind}` : ""}
                  {rule.threshold ? ` · threshold ${rule.threshold}` : ""}
                </p>
              </article>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
