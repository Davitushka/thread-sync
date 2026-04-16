import React, { useEffect, useState, useCallback } from "react";
import {
  isTauriSync,
  checkStackStatus,
  fetchObservabilitySnapshot,
  dockerComposeAction,
  listAttacks,
  runAttack,
  type ServiceStatus,
  type ObsSnapshot,
  type AttackDef,
  type AttackResult,
} from "../tauri-bridge";

// ── Styles ───────────────────────────────────────────────────────────────────

const panelStyle: React.CSSProperties = {
  position: "fixed",
  right: 0,
  top: 0,
  bottom: 0,
  width: 370,
  background: "rgba(15, 20, 30, 0.97)",
  borderLeft: "1px solid rgba(100, 120, 180, 0.2)",
  zIndex: 9999,
  overflowY: "auto",
  fontFamily: "'Inter', 'Segoe UI', system-ui, sans-serif",
  color: "#d0d8e8",
  fontSize: 13,
  transition: "transform 0.25s ease",
};

const sectionStyle: React.CSSProperties = {
  padding: "12px 16px",
  borderBottom: "1px solid rgba(100, 120, 180, 0.12)",
};

const btn: React.CSSProperties = {
  padding: "6px 14px",
  borderRadius: 6,
  border: "1px solid rgba(100, 120, 180, 0.3)",
  background: "rgba(60, 80, 140, 0.25)",
  color: "#b0c0e0",
  cursor: "pointer",
  fontSize: 12,
  marginRight: 6,
  marginBottom: 4,
};

const btnGreen: React.CSSProperties = { ...btn, background: "rgba(40, 120, 60, 0.35)", borderColor: "rgba(60, 180, 90, 0.4)", color: "#80e090" };
const btnRed: React.CSSProperties = { ...btn, background: "rgba(200, 40, 40, 0.3)", borderColor: "rgba(255, 60, 60, 0.4)", color: "#ff8080" };
const btnBlue: React.CSSProperties = { ...btn, background: "rgba(40, 100, 220, 0.4)", borderColor: "rgba(60, 130, 255, 0.5)", color: "#90c0ff" };

// ── Stack Control ────────────────────────────────────────────────────────────

function StackControl() {
  const [services, setServices] = useState<ServiceStatus[]>([]);
  const [loading, setLoading] = useState(false);
  const [dockerOut, setDockerOut] = useState("");

  const refresh = useCallback(async () => {
    setLoading(true);
    const status = await checkStackStatus();
    if (status) setServices(status.services);
    setLoading(false);
  }, []);

  const docker = useCallback(async (action: "start" | "stop" | "restart" | "status") => {
    setLoading(true);
    const result = await dockerComposeAction(action);
    if (result !== null) setDockerOut(result);
    setLoading(false);
  }, []);

  return (
    <div style={sectionStyle}>
      <h3 style={{ margin: "0 0 8px", fontSize: 14, color: "#90a8d0" }}>Stack Control</h3>
      <div style={{ marginBottom: 8, display: "flex", flexWrap: "wrap", gap: 4 }}>
        <button style={btnBlue} onClick={refresh} disabled={loading}>
          {loading ? "..." : "Health"}
        </button>
        <button style={btnGreen} onClick={() => docker("start")} disabled={loading}>
          Start
        </button>
        <button style={btnRed} onClick={() => docker("stop")} disabled={loading}>
          Stop
        </button>
        <button style={btn} onClick={() => docker("restart")} disabled={loading}>
          Restart
        </button>
        <button style={btn} onClick={() => docker("status")} disabled={loading}>
          docker ps
        </button>
      </div>

      {services.length > 0 && (
        <table style={{ width: "100%", borderCollapse: "collapse", marginTop: 4 }}>
          <tbody>
            {services.map((s) => (
              <tr key={s.name}>
                <td style={{ padding: "3px 0" }}>
                  <span
                    style={{
                      display: "inline-block",
                      width: 8,
                      height: 8,
                      borderRadius: "50%",
                      background: s.healthy ? "#4caf50" : "#f44336",
                      marginRight: 8,
                      boxShadow: s.healthy ? "0 0 6px rgba(76,175,80,0.5)" : "none",
                    }}
                  />
                  {s.name}
                </td>
                <td style={{ padding: "3px 0", textAlign: "right", color: s.healthy ? "#80c080" : "#ff8080", fontWeight: 600 }}>
                  {s.healthy ? "UP" : "DOWN"}
                </td>
              </tr>
            ))}
          </tbody>
        </table>
      )}

      {dockerOut && (
        <pre
          style={{
            marginTop: 8,
            padding: 8,
            background: "rgba(0,0,0,0.3)",
            borderRadius: 4,
            fontSize: 11,
            maxHeight: 180,
            overflow: "auto",
            whiteSpace: "pre-wrap",
            color: "#90a0b8",
          }}
        >
          {dockerOut.slice(-2000)}
        </pre>
      )}
    </div>
  );
}

// ── Observability ────────────────────────────────────────────────────────────

function ObservabilityPanel() {
  const [obs, setObs] = useState<ObsSnapshot | null>(null);

  const refresh = useCallback(async () => {
    const snapshot = await fetchObservabilitySnapshot("http://127.0.0.1:8091");
    if (snapshot) setObs(snapshot);
  }, []);

  return (
    <div style={sectionStyle}>
      <h3 style={{ margin: "0 0 8px", fontSize: 14, color: "#90a8d0" }}>Observability</h3>
      <button style={btnBlue} onClick={refresh}>Refresh</button>
      {obs && (
        <div style={{ marginTop: 8, lineHeight: 1.6 }}>
          <div><strong>Prometheus</strong> {obs.prom_version} &mdash; {obs.prom_up_targets}/{obs.prom_total_targets} targets</div>
          <div><strong>Alertmanager</strong> {obs.am_alerts_active} active, {obs.am_alerts_silenced} silenced</div>
          <div style={{ fontSize: 11, color: "#556", marginTop: 4 }}>{obs.fetched_at}</div>
        </div>
      )}
    </div>
  );
}

// ── Attack Lab ───────────────────────────────────────────────────────────────

function AttackLab() {
  const [attacks, setAttacks] = useState<AttackDef[]>([]);
  const [result, setResult] = useState<AttackResult | null>(null);
  const [running, setRunning] = useState(false);

  useEffect(() => {
    listAttacks().then((a) => { if (a) setAttacks(a); });
  }, []);

  const fire = useCallback(async (idx: number) => {
    setRunning(true);
    setResult(null);
    const r = await runAttack(idx);
    if (r) setResult(r);
    setRunning(false);
  }, []);

  return (
    <div style={sectionStyle}>
      <h3 style={{ margin: "0 0 8px", fontSize: 14, color: "#90a8d0" }}>Attack Lab</h3>
      {attacks.length === 0 && (
        <p style={{ color: "#667", fontSize: 12 }}>Loading attacks...</p>
      )}
      {attacks.map((a, i) => (
        <div
          key={a.rule_id}
          style={{
            display: "flex",
            alignItems: "center",
            padding: "5px 0",
            borderBottom: "1px solid rgba(100,120,180,0.08)",
          }}
        >
          <button
            style={{
              ...btn,
              padding: "3px 10px",
              background: a.severity === "high" ? "rgba(200,40,40,0.3)" : "rgba(200,140,40,0.25)",
              borderColor: a.severity === "high" ? "rgba(255,60,60,0.4)" : "rgba(255,180,60,0.3)",
              color: a.severity === "high" ? "#ff9090" : "#ffc060",
              fontWeight: 700,
              marginRight: 10,
              minWidth: 48,
            }}
            onClick={() => fire(i)}
            disabled={running}
          >
            FIRE
          </button>
          <div style={{ flex: 1 }}>
            <div style={{ fontWeight: 600, fontSize: 12 }}>{a.name}</div>
            <div style={{ fontSize: 10, color: "#678" }}>
              {a.mitre} &middot; {a.events} events &middot; {a.description}
            </div>
          </div>
        </div>
      ))}
      {result && (
        <div
          style={{
            marginTop: 8,
            padding: 8,
            background: result.success ? "rgba(40,120,40,0.2)" : "rgba(120,40,40,0.2)",
            borderRadius: 4,
            fontSize: 12,
          }}
        >
          {result.attack_name}: {result.events_sent} events &mdash;{" "}
          {result.success ? "OK" : `FAIL: ${result.error}`}
        </div>
      )}
    </div>
  );
}

// ── Main component ───────────────────────────────────────────────────────────

export default function TauriPanel() {
  const [open, setOpen] = useState(false);
  const [inTauri, setInTauri] = useState(false);

  useEffect(() => {
    setInTauri(isTauriSync());
  }, []);

  if (!inTauri) return null;

  return (
    <>
      {/* Toggle button */}
      <button
        onClick={() => setOpen(!open)}
        style={{
          position: "fixed",
          right: open ? 370 : 0,
          top: 12,
          zIndex: 10000,
          padding: "8px 12px",
          borderRadius: open ? "6px 0 0 6px" : "6px",
          border: "1px solid rgba(100,120,180,0.3)",
          background: "rgba(15,20,30,0.95)",
          color: "#90a8d0",
          cursor: "pointer",
          fontSize: 18,
          lineHeight: 1,
          transition: "right 0.25s ease",
        }}
        title="SIEM Desktop Panel"
      >
        {open ? "\u2715" : "\u2699"}
      </button>

      {/* Slide-out panel */}
      <div
        style={{
          ...panelStyle,
          transform: open ? "translateX(0)" : "translateX(100%)",
        }}
      >
        <div style={{ padding: "14px 16px", borderBottom: "1px solid rgba(100,120,180,0.12)" }}>
          <strong style={{ fontSize: 15, color: "#a0b8e0" }}>SIEM-Lite Desktop</strong>
          <div style={{ fontSize: 11, color: "#556" }}>Tauri + React</div>
        </div>
        <StackControl />
        <ObservabilityPanel />
        <AttackLab />
      </div>
    </>
  );
}
