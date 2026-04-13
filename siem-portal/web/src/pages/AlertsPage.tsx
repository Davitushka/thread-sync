import { useEffect, useMemo, useState } from "react";
import { createCase, getAlerts, type AlertItem } from "../api";

function severity(value?: string) {
  return (value || "unknown").toLowerCase();
}

export default function AlertsPage() {
  const [alerts, setAlerts] = useState<AlertItem[]>([]);
  const [actor, setActor] = useState(() => localStorage.getItem("soc_actor") || "analyst");
  const [err, setErr] = useState<string | null>(null);
  const [creating, setCreating] = useState<string | null>(null);

  const load = () =>
    getAlerts()
      .then(setAlerts)
      .catch((e) => setErr(String(e)));

  useEffect(() => {
    load();
  }, []);

  const active = useMemo(
    () => alerts.filter((a) => (a.status?.state || "").toLowerCase() !== "suppressed"),
    [alerts]
  );

  const promote = async (alert: AlertItem) => {
    setCreating(alert.fingerprint);
    setErr(null);
    localStorage.setItem("soc_actor", actor);
    try {
      await createCase(
        {
          title: alert.labels.alertname || "Alert",
          description: alert.annotations?.description || alert.annotations?.summary || "Promoted from alert inbox",
          severity: severity(alert.labels.severity),
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
    <div className="page-grid">
      <section className="card">
        <h2>Alert inbox</h2>
        <p className="meta">
          Активные алерты Alertmanager через портал. Из этой страницы можно быстро создать кейс и продолжить triage.
        </p>
        <label className="meta" style={{ display: "block", marginBottom: "0.75rem" }}>
          Analyst
          <input value={actor} onChange={(e) => setActor(e.target.value)} style={{ width: "220px", marginTop: "0.25rem" }} />
        </label>
        {err && <p className="error">{err}</p>}
        {!alerts.length ? (
          <p className="meta">Загрузка или пустой inbox…</p>
        ) : (
          <div className="alert-stack">
            {active.map((alert) => (
              <article key={alert.fingerprint} className="alert-card">
                <header>
                  <span className={`badge sev-${severity(alert.labels.severity)}`}>{severity(alert.labels.severity)}</span>
                  <h3>{alert.labels.alertname || "Alert"}</h3>
                </header>
                <p className="meta">
                  Fingerprint: <code>{alert.fingerprint}</code>
                </p>
                <p className="alert-desc">{alert.annotations?.description || alert.annotations?.summary || "—"}</p>
                <p className="meta">
                  Source: {alert.labels.instance || alert.labels.job || "—"} · Started:{" "}
                  {alert.startsAt ? new Date(alert.startsAt).toLocaleString() : "—"}
                </p>
                <div className="btn-row tight">
                  <button type="button" onClick={() => promote(alert)} disabled={creating === alert.fingerprint}>
                    {creating === alert.fingerprint ? "Creating…" : "Promote to case"}
                  </button>
                </div>
              </article>
            ))}
          </div>
        )}
      </section>
    </div>
  );
}
