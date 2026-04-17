import React, { type ReactNode, useEffect, useState, useCallback } from "react";
import {
  isTauriSync,
  checkStackStatus,
  fetchObservabilitySnapshot,
  dockerComposeAction,
  listAttacks,
  runAttack,
  getSettings,
  saveSettings,
  type ServiceStatus,
  type ObsSnapshot,
  type AttackDef,
  type AttackResult,
  type AppSettings,
} from "../tauri-bridge";

// ── Theme tokens (matches old egui operator) ────────────────────────────────

const T = {
  bg:          "rgb(14, 18, 24)",
  bgCard:      "rgb(24, 30, 42)",
  bgCardHover: "rgb(30, 38, 52)",
  bgInput:     "rgb(18, 22, 32)",
  border:      "rgb(46, 58, 79)",
  borderLight: "rgb(36, 45, 62)",
  borderFocus: "rgb(90, 180, 240)",

  text:        "rgb(175, 185, 200)",
  textMuted:   "rgb(120, 128, 145)",
  textBright:  "rgb(225, 232, 242)",
  textWhite:   "#ffffff",

  accent:      "rgb(120, 190, 255)",
  accentBg:    "rgba(120, 190, 255, 0.08)",
  green:       "rgb(90, 200, 140)",
  greenBg:     "rgba(90, 200, 140, 0.08)",
  red:         "rgb(235, 75, 85)",
  redBg:       "rgba(235, 75, 85, 0.08)",
  orange:      "rgb(245, 140, 70)",
  orangeBg:    "rgba(245, 140, 70, 0.08)",
  yellow:      "rgb(235, 195, 80)",
  yellowBg:    "rgba(235, 195, 80, 0.08)",
  blue:        "rgb(70, 120, 210)",
  blueBg:      "rgba(70, 120, 210, 0.10)",

  sidebarBg:   "rgb(14, 18, 24)",
  sidebarBorder: "rgb(32, 40, 54)",
  navBg:       "rgb(28, 34, 46)",
  navHover:    "rgb(42, 52, 70)",
  navActive:   "rgba(36, 92, 135, 0.55)",
  navActiveBorder: "rgb(90, 180, 240)",

  radius:      8,
  radiusSm:    6,
  radiusLg:    12,
  font:        "'Inter', 'Segoe UI', system-ui, -apple-system, sans-serif",
  mono:        "'JetBrains Mono', 'Fira Code', 'Cascadia Code', monospace",
  transition:  "all 0.18s ease",
} as const;

// ── Inline SVG icons (no emoji, no extra deps) ───────────────────────────────

function IcSvg({ size, children }: { size: number; children: React.ReactNode }) {
  return (
    <svg width={size} height={size} viewBox="0 0 24 24" aria-hidden style={{ display: "block", flexShrink: 0 }}>
      {children}
    </svg>
  );
}

function IcLayoutGrid({ size = 16 }: { size?: number }) {
  return (
    <IcSvg size={size}>
      <g fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round">
        <rect x="3" y="3" width="7.5" height="7.5" rx="1.5" />
        <rect x="13.5" y="3" width="7.5" height="7.5" rx="1.5" />
        <rect x="3" y="13.5" width="7.5" height="7.5" rx="1.5" />
        <rect x="13.5" y="13.5" width="7.5" height="7.5" rx="1.5" />
      </g>
    </IcSvg>
  );
}

function IcLayers({ size = 16 }: { size?: number }) {
  return (
    <IcSvg size={size}>
      <g fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round">
        <path d="M12 4.5 4.5 8.25 12 12l7.5-3.75L12 4.5Z" />
        <path d="m4.5 12 7.5 3.75L19.5 12" />
        <path d="m4.5 15.75 7.5 3.75 7.5-3.75" />
      </g>
    </IcSvg>
  );
}

function IcActivity({ size = 16 }: { size?: number }) {
  return (
    <IcSvg size={size}>
      <path
        d="M4 12h4l2-7 4 14 2-7h4"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.75"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </IcSvg>
  );
}

function IcZap({ size = 16 }: { size?: number }) {
  return (
    <IcSvg size={size}>
      <path
        d="M13 2.5 4 14.5h7l-1 7 9-12h-7l1-7Z"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.75"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </IcSvg>
  );
}

function IcSettings({ size = 16 }: { size?: number }) {
  return (
    <IcSvg size={size}>
      <g fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round">
        <circle cx="12" cy="12" r="3" />
        <path d="M12 1.5v2.2M12 20.3v2.2M4.2 12H2M22 12h-2.2M5.6 5.6 4.1 4.1M19.9 19.9l-1.5-1.5M5.6 18.4l-1.5 1.5M19.9 4.1l-1.5 1.5" />
      </g>
    </IcSvg>
  );
}

function IcRefresh({ size = 14 }: { size?: number }) {
  return (
    <IcSvg size={size}>
      <g fill="none" stroke="currentColor" strokeWidth="1.75" strokeLinecap="round" strokeLinejoin="round">
        <path d="M21 12a9 9 0 0 1-15.6 5.4" />
        <path d="M3 12a9 9 0 0 1 15.6-5.4" />
        <path d="M3.5 8.5V4h4.5" />
        <path d="M20.5 15.5V20h-4.5" />
      </g>
    </IcSvg>
  );
}

function IcPlay({ size = 14 }: { size?: number }) {
  return (
    <IcSvg size={size}>
      <path d="M9.5 7.5v9L17 12l-7.5-4.5Z" fill="currentColor" stroke="none" />
    </IcSvg>
  );
}

function IcSquare({ size = 14 }: { size?: number }) {
  return (
    <IcSvg size={size}>
      <rect x="6" y="6" width="12" height="12" rx="1.5" fill="currentColor" stroke="none" />
    </IcSvg>
  );
}

function IcPulse({ size = 14 }: { size?: number }) {
  return (
    <IcSvg size={size}>
      <path
        d="M4 12h3l2-5 3 10 2-5h6"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.75"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </IcSvg>
  );
}

function IcCheck({ size = 14 }: { size?: number }) {
  return (
    <IcSvg size={size}>
      <path
        d="M5 12.5 9.5 17 19 7"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.75"
        strokeLinecap="round"
        strokeLinejoin="round"
      />
    </IcSvg>
  );
}

function IcX({ size = 14 }: { size?: number }) {
  return (
    <IcSvg size={size}>
      <path
        d="M6 6l12 12M18 6 6 18"
        fill="none"
        stroke="currentColor"
        strokeWidth="1.75"
        strokeLinecap="round"
      />
    </IcSvg>
  );
}

function IcLoader({ size = 14 }: { size?: number }) {
  return (
    <IcSvg size={size}>
      <g style={{ transformOrigin: "12px 12px", animation: "tauri-panel-spin 0.8s linear infinite" }}>
        <circle
          cx="12"
          cy="12"
          r="9"
          fill="none"
          stroke="currentColor"
          strokeWidth="2"
          strokeDasharray="14 32"
          strokeLinecap="round"
        />
      </g>
    </IcSvg>
  );
}

// ── Section enum ────────────────────────────────────────────────────────────

type SectionId = "overview" | "stack" | "obs" | "attacks" | "settings";

interface NavItem {
  id: SectionId;
  label: string;
  subtitle: string;
  icon: ReactNode;
}

const NAV_ITEMS: NavItem[] = [
  { id: "overview", label: "Stack",   subtitle: "KPI и SLA",      icon: <IcLayoutGrid size={16} /> },
  { id: "stack",    label: "Stack",   subtitle: "Docker health",  icon: <IcLayers size={16} /> },
  { id: "obs",      label: "Obs",     subtitle: "Prometheus",     icon: <IcActivity size={16} /> },
  { id: "attacks",  label: "Attacks", subtitle: "MITRE ATT&CK",   icon: <IcZap size={16} /> },
  { id: "settings", label: "Config",  subtitle: "Connection",     icon: <IcSettings size={16} /> },
];

// ── Shared primitives ───────────────────────────────────────────────────────

function Card({ children, border }: { children: ReactNode; border?: string }) {
  return (
    <div style={{
      padding: "14px 16px", borderRadius: T.radiusLg,
      background: T.bgCard, border: `1px solid ${border || T.border}`,
      marginBottom: 10,
    }}>
      {children}
    </div>
  );
}

function KpiCard({ label, value, color, bg }: { label: string; value: string; color: string; bg: string }) {
  return (
    <div style={{
      padding: "10px 14px", borderRadius: T.radius,
      background: bg, border: `1px solid ${color}25`,
      textAlign: "center", minWidth: 0, flex: "1 1 0",
    }}>
      <div style={{ fontSize: 20, fontWeight: 800, color, lineHeight: 1.2 }}>{value}</div>
      <div style={{ fontSize: 10, fontWeight: 600, color: T.textMuted, marginTop: 4, textTransform: "uppercase", letterSpacing: "0.05em" }}>{label}</div>
    </div>
  );
}

function PillLabel({ text, color }: { text: string; color: string }) {
  return (
    <span style={{
      display: "inline-block", padding: "2px 8px", borderRadius: 4,
      background: `${color}20`, color, fontSize: 9, fontWeight: 700,
      fontFamily: T.mono, textTransform: "uppercase", letterSpacing: "0.05em",
    }}>
      {text}
    </span>
  );
}

function ActionBtn({
  label, icon, variant = "default", disabled, onClick, style,
}: {
  label: string; icon?: ReactNode; variant?: "default" | "green" | "red" | "blue" | "accent"; disabled?: boolean;
  onClick: () => void; style?: React.CSSProperties;
}) {
  const vMap = {
    default: { bg: T.navBg, border: T.border, color: T.text },
    green:   { bg: T.greenBg, border: `${T.green}40`, color: T.green },
    red:     { bg: T.redBg,   border: `${T.red}40`, color: T.red },
    blue:    { bg: T.blueBg,  border: `${T.blue}40`, color: T.blue },
    accent:  { bg: T.accentBg, border: `${T.accent}30`, color: T.accent },
  };
  const v = vMap[variant];
  return (
    <button
      onClick={onClick} disabled={disabled}
      style={{
        padding: "7px 14px", borderRadius: T.radiusSm,
        border: `1px solid ${v.border}`, background: v.bg,
        color: v.color, cursor: disabled ? "not-allowed" : "pointer",
        fontSize: 12, fontWeight: 600, fontFamily: T.font,
        opacity: disabled ? 0.4 : 1, transition: T.transition,
        display: "inline-flex", alignItems: "center", gap: 5,
        ...style,
      }}
    >
      {icon && <span style={{ display: "inline-flex", alignItems: "center", lineHeight: 0 }}>{icon}</span>}
      {label}
    </button>
  );
}

function PulseDot({ color, glowing }: { color: string; glowing?: boolean }) {
  return (
    <span style={{
      display: "inline-block", width: 8, height: 8, borderRadius: "50%",
      background: color, flexShrink: 0,
      boxShadow: glowing ? `0 0 6px ${color}60` : "none",
    }} />
  );
}

function FieldLabel({ children }: { children: ReactNode }) {
  return (
    <div style={{ fontSize: 10, fontWeight: 600, color: T.textMuted, marginBottom: 4, textTransform: "uppercase", letterSpacing: "0.06em" }}>
      {children}
    </div>
  );
}

function Toggle({ checked, onChange, label }: { checked: boolean; onChange: (v: boolean) => void; label: string }) {
  return (
    <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", padding: "6px 0" }}>
      <span style={{ fontSize: 12, color: T.text }}>{label}</span>
      <button
        onClick={() => onChange(!checked)}
        style={{
          width: 36, height: 20, borderRadius: 10, border: "none", cursor: "pointer",
          background: checked ? T.accent : "rgba(120,128,145,0.2)", transition: T.transition,
          position: "relative",
        }}
      >
        <span style={{
          position: "absolute", top: 2, left: checked ? 18 : 2,
          width: 16, height: 16, borderRadius: "50%", background: "#fff", transition: T.transition,
        }} />
      </button>
    </div>
  );
}

// ── Overview Panel ──────────────────────────────────────────────────────────

function OverviewPanel({ onNavigate }: { onNavigate: (s: SectionId) => void }) {
  const [services, setServices] = useState<ServiceStatus[]>([]);
  const [obs, setObs] = useState<ObsSnapshot | null>(null);
  const [attacks, setAttacks] = useState<AttackDef[]>([]);
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [loading, setLoading] = useState(false);
  const [dockerOut, setDockerOut] = useState("");

  const refreshAll = useCallback(async () => {
    setLoading(true);
    const [s, o, a, cfg] = await Promise.all([
      checkStackStatus(),
      fetchObservabilitySnapshot("http://127.0.0.1:8091"),
      listAttacks(),
      getSettings(),
    ]);
    if (s) setServices(s.services);
    if (o) setObs(o);
    if (a) setAttacks(a);
    if (cfg) setSettings(cfg);
    setLoading(false);
  }, []);

  const docker = useCallback(async (action: "start" | "stop" | "restart" | "status") => {
    setLoading(true);
    const result = await dockerComposeAction(action);
    if (result !== null) setDockerOut(result);
    // refresh health after action
    const s = await checkStackStatus();
    if (s) setServices(s.services);
    setLoading(false);
  }, []);

  useEffect(() => { refreshAll(); }, [refreshAll]);

  const upCount = services.filter(s => s.healthy).length;
  const totalCount = services.length;
  const allUp = totalCount > 0 && upCount === totalCount;
  const critCount = attacks.filter(a => a.severity === "critical").length;
  const highCount = attacks.filter(a => a.severity === "high").length;

  return (
    <div style={{ padding: "0 16px 20px" }}>
      {/* Header */}
      <Card>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 8 }}>
          <div>
            <div style={{ fontSize: 22, fontWeight: 800, color: T.textWhite, lineHeight: 1.2 }}>SOC Overview</div>
            <div style={{ fontSize: 11, color: T.textMuted, marginTop: 3 }}>Live posture, triage pressure, SLA and stack control</div>
          </div>
          <ActionBtn label="Refresh All" icon={<IcRefresh size={14} />} variant="accent" disabled={loading} onClick={refreshAll} />
        </div>
        {settings && (
          <div style={{ display: "flex", gap: 12, flexWrap: "wrap", fontSize: 11, color: T.textMuted }}>
            <span>{settings.whoami} ({settings.role})</span>
            {settings.auto_refresh_enabled && <span>Auto-refresh: {settings.auto_refresh_interval_sec}s</span>}
          </div>
        )}
      </Card>

      {/* Docker Stack Control */}
      <Card border={T.blue}>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 8 }}>
          <div style={{ fontSize: 16, fontWeight: 700, color: T.textBright }}>Docker Stack Control</div>
          <ActionBtn label="Go to Stack" variant="blue" onClick={() => onNavigate("stack")} style={{ fontSize: 10, padding: "4px 10px" }} />
        </div>
        <div style={{ fontSize: 11, color: T.textMuted, marginBottom: 10 }}>
          Запуск и остановка всего SIEM-стека прямо из Desktop.
        </div>
        <div style={{ display: "flex", gap: 6, flexWrap: "wrap", marginBottom: 10 }}>
          <ActionBtn label="Start" icon={<IcPlay size={13} />} variant="green" disabled={loading} onClick={() => docker("start")} />
          <ActionBtn label="Stop Stack" icon={<IcSquare size={12} />} variant="red" disabled={loading} onClick={() => docker("stop")} />
          <ActionBtn label="Restart" icon={<IcRefresh size={14} />} disabled={loading} onClick={() => docker("restart")} />
          <ActionBtn label="Status" disabled={loading} onClick={() => docker("status")} style={{ fontFamily: T.mono, fontSize: 11 }} />
        </div>

        {/* Health summary */}
        {totalCount > 0 && (
          <div style={{
            display: "flex", alignItems: "center", gap: 10,
            padding: "8px 12px", borderRadius: T.radiusSm,
            background: allUp ? T.greenBg : T.redBg,
            border: `1px solid ${allUp ? `${T.green}25` : `${T.red}25`}`,
            marginBottom: 8,
          }}>
            <PulseDot color={allUp ? T.green : T.red} glowing={allUp} />
            <span style={{ fontSize: 14, fontWeight: 800, color: allUp ? T.green : T.red }}>{upCount}/{totalCount}</span>
            <span style={{ fontSize: 11, color: T.textMuted }}>services healthy</span>
          </div>
        )}

        {dockerOut && (
          <pre style={{
            marginTop: 6, padding: "8px 10px", background: T.bgInput, borderRadius: T.radiusSm,
            border: `1px solid ${T.borderLight}`, fontSize: 10, fontFamily: T.mono, lineHeight: 1.6,
            maxHeight: 100, overflow: "auto", whiteSpace: "pre-wrap", color: T.textMuted,
          }}>
            {dockerOut.slice(-1500)}
          </pre>
        )}
      </Card>

      {/* KPI row */}
      <div style={{ display: "flex", gap: 8, marginBottom: 10 }}>
        <KpiCard label="Stack" value={totalCount > 0 ? `${upCount}/${totalCount}` : "--"} color={allUp ? T.green : T.red} bg={allUp ? T.greenBg : T.redBg} />
        <KpiCard label="Targets" value={obs ? `${obs.prom_up_targets}/${obs.prom_total_targets}` : "--"} color={T.accent} bg={T.accentBg} />
        <KpiCard label="Alerts" value={obs ? String(obs.am_alerts_active) : "--"} color={obs && obs.am_alerts_active > 0 ? T.orange : T.green} bg={obs && obs.am_alerts_active > 0 ? T.orangeBg : T.greenBg} />
      </div>

      {/* Second KPI row */}
      <div style={{ display: "flex", gap: 8, marginBottom: 10 }}>
        <KpiCard label="Critical" value={String(critCount)} color={T.red} bg={T.redBg} />
        <KpiCard label="High" value={String(highCount)} color={T.orange} bg={T.orangeBg} />
        <KpiCard label="Attacks" value={String(attacks.length)} color={T.accent} bg={T.accentBg} />
      </div>

      {/* Attack Lab summary */}
      <Card>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 6 }}>
          <div style={{ fontSize: 14, fontWeight: 700, color: T.textBright }}>Attack Lab</div>
          <ActionBtn label="Go to Attacks" variant="accent" onClick={() => onNavigate("attacks")} style={{ fontSize: 10, padding: "4px 10px" }} />
        </div>
        <div style={{ fontSize: 11, color: T.textMuted }}>
          {attacks.length > 0
            ? `${critCount} critical, ${highCount} high, ${attacks.filter(a => a.severity === "medium").length} medium available`
            : "Loading attacks..."}
        </div>
      </Card>

      {/* Observability summary */}
      <Card>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 6 }}>
          <div style={{ fontSize: 14, fontWeight: 700, color: T.textBright }}>Observability</div>
          <ActionBtn label="Details" variant="blue" onClick={() => onNavigate("obs")} style={{ fontSize: 10, padding: "4px 10px" }} />
        </div>
        {obs ? (
          <div style={{ display: "flex", gap: 16 }}>
            <div>
              <span style={{ fontSize: 11, color: T.textMuted }}>Prometheus </span>
              <span style={{ fontSize: 13, fontWeight: 700, color: T.accent }}>{obs.prom_up_targets}/{obs.prom_total_targets}</span>
            </div>
            <div>
              <span style={{ fontSize: 11, color: T.textMuted }}>Alerts active </span>
              <span style={{ fontSize: 13, fontWeight: 700, color: obs.am_alerts_active > 0 ? T.orange : T.green }}>{obs.am_alerts_active}</span>
            </div>
          </div>
        ) : (
          <div style={{ fontSize: 11, color: T.textMuted }}>Loading...</div>
        )}
      </Card>

      {/* Config summary */}
      <Card>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 6 }}>
          <div style={{ fontSize: 14, fontWeight: 700, color: T.textBright }}>Configuration</div>
          <ActionBtn label="Edit" variant="accent" onClick={() => onNavigate("settings")} style={{ fontSize: 10, padding: "4px 10px" }} />
        </div>
        <div style={{ display: "flex", flexDirection: "column", gap: 3, fontSize: 11, color: T.textMuted }}>
          <div>API: <span style={{ color: T.text, fontFamily: T.mono }}>{settings?.api_base || "..."}</span></div>
          <div>User: <span style={{ color: T.text }}>{settings?.whoami || "..."}</span> <span style={{ color: T.textMuted }}>({settings?.role || "..."})</span></div>
          <div>Detection: <span style={{ color: T.text, fontFamily: T.mono }}>{settings?.detection_engine_url || "..."}</span></div>
        </div>
      </Card>
    </div>
  );
}

// ── Stack Control Panel ─────────────────────────────────────────────────────

function StackControlPanel() {
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
    const status = await checkStackStatus();
    if (status) setServices(status.services);
    setLoading(false);
  }, []);

  useEffect(() => { refresh(); }, [refresh]);

  const upCount = services.filter(s => s.healthy).length;
  const totalCount = services.length;
  const allUp = totalCount > 0 && upCount === totalCount;

  return (
    <div style={{ padding: "0 16px 20px" }}>
      <Card border={T.blue}>
        <div style={{ fontSize: 18, fontWeight: 700, color: T.textBright, marginBottom: 6 }}>Docker Stack Control</div>
        <div style={{ fontSize: 11, color: T.textMuted, marginBottom: 12 }}>
          Запуск и остановка всего SIEM-стека.
        </div>
        <div style={{ display: "flex", gap: 6, flexWrap: "wrap", marginBottom: 12 }}>
          <ActionBtn label="Health" icon={<IcPulse size={14} />} variant="blue" disabled={loading} onClick={refresh} />
          <ActionBtn label="Start" icon={<IcPlay size={13} />} variant="green" disabled={loading} onClick={() => docker("start")} />
          <ActionBtn label="Stop Stack" icon={<IcSquare size={12} />} variant="red" disabled={loading} onClick={() => docker("stop")} />
          <ActionBtn label="Restart Stack" icon={<IcRefresh size={14} />} disabled={loading} onClick={() => docker("restart")} />
          <ActionBtn label="docker ps" disabled={loading} onClick={() => docker("status")} style={{ fontFamily: T.mono, fontSize: 11 }} />
        </div>

        {totalCount > 0 && (
          <div style={{
            display: "flex", alignItems: "center", gap: 10,
            padding: "10px 14px", borderRadius: T.radiusSm,
            background: allUp ? T.greenBg : T.redBg,
            border: `1px solid ${allUp ? `${T.green}25` : `${T.red}25`}`,
            marginBottom: 10,
          }}>
            <PulseDot color={allUp ? T.green : T.red} glowing={allUp} />
            <span style={{ fontSize: 18, fontWeight: 800, color: allUp ? T.green : T.red }}>{upCount}/{totalCount}</span>
            <span style={{ fontSize: 12, color: T.textMuted }}>services healthy</span>
          </div>
        )}

        {services.length > 0 && (
          <div style={{
            borderRadius: T.radiusSm, border: `1px solid ${T.borderLight}`, overflow: "hidden",
          }}>
            {services.map((s, i) => (
              <div key={s.name} style={{
                display: "flex", alignItems: "center", justifyContent: "space-between",
                padding: "8px 14px",
                background: i % 2 === 0 ? "transparent" : "rgba(46,58,79,0.15)",
                borderBottom: i < services.length - 1 ? `1px solid ${T.borderLight}` : "none",
              }}>
                <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                  <PulseDot color={s.healthy ? T.green : T.red} glowing={s.healthy} />
                  <span style={{ fontSize: 12, color: T.text, fontWeight: 500 }}>{s.name}</span>
                </div>
                <span style={{ fontSize: 10, fontWeight: 700, fontFamily: T.mono, color: s.healthy ? T.green : T.red }}>
                  {s.healthy ? "UP" : "DOWN"}
                </span>
              </div>
            ))}
          </div>
        )}
      </Card>

      {dockerOut && (
        <Card>
          <div style={{ fontSize: 12, fontWeight: 700, color: T.textMuted, marginBottom: 6, textTransform: "uppercase", letterSpacing: "0.05em" }}>Output</div>
          <pre style={{
            padding: "10px 12px", background: T.bgInput, borderRadius: T.radiusSm,
            border: `1px solid ${T.borderLight}`, fontSize: 10, fontFamily: T.mono, lineHeight: 1.6,
            maxHeight: 200, overflow: "auto", whiteSpace: "pre-wrap", color: T.textMuted,
          }}>
            {dockerOut.slice(-2000)}
          </pre>
        </Card>
      )}
    </div>
  );
}

// ── Observability Panel ─────────────────────────────────────────────────────

function ObservabilityPanel() {
  const [obs, setObs] = useState<ObsSnapshot | null>(null);

  const refresh = useCallback(async () => {
    const snapshot = await fetchObservabilitySnapshot("http://127.0.0.1:8091");
    if (snapshot) setObs(snapshot);
  }, []);

  useEffect(() => { refresh(); }, [refresh]);

  return (
    <div style={{ padding: "0 16px 20px" }}>
      <Card>
        <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 10 }}>
          <div style={{ fontSize: 18, fontWeight: 700, color: T.textBright }}>Observability</div>
          <ActionBtn label="Refresh" icon={<IcRefresh size={14} />} variant="blue" onClick={refresh} />
        </div>

        {obs ? (
          <div style={{ display: "flex", flexDirection: "column", gap: 10 }}>
            {/* Prometheus */}
            <div style={{
              padding: "12px 14px", borderRadius: T.radius,
              background: T.blueBg, border: `1px solid ${T.blue}20`,
            }}>
              <div style={{ display: "flex", alignItems: "center", justifyContent: "space-between", marginBottom: 6 }}>
                <span style={{ fontSize: 13, fontWeight: 700, color: T.accent }}>Prometheus</span>
                <span style={{ fontSize: 10, fontFamily: T.mono, color: T.textMuted }}>v{obs.prom_version}</span>
              </div>
              <div style={{ display: "flex", alignItems: "baseline", gap: 4 }}>
                <span style={{ fontSize: 26, fontWeight: 800, color: T.textBright }}>{obs.prom_up_targets}</span>
                <span style={{ fontSize: 13, color: T.textMuted }}>/ {obs.prom_total_targets} targets</span>
              </div>
              <div style={{ marginTop: 8, height: 4, borderRadius: 2, background: "rgba(120,190,255,0.12)" }}>
                <div style={{
                  height: "100%", borderRadius: 2,
                  width: `${obs.prom_total_targets ? (obs.prom_up_targets / obs.prom_total_targets * 100) : 0}%`,
                  background: T.accent, transition: "width 0.4s ease",
                }} />
              </div>
            </div>

            {/* Alertmanager */}
            <div style={{
              padding: "12px 14px", borderRadius: T.radius,
              background: obs.am_alerts_active > 0 ? T.orangeBg : T.greenBg,
              border: `1px solid ${obs.am_alerts_active > 0 ? `${T.orange}20` : `${T.green}20`}`,
            }}>
              <div style={{ fontSize: 13, fontWeight: 700, color: obs.am_alerts_active > 0 ? T.orange : T.green, marginBottom: 6 }}>
                Alertmanager
              </div>
              <div style={{ display: "flex", gap: 20 }}>
                <div>
                  <span style={{ fontSize: 22, fontWeight: 800, color: obs.am_alerts_active > 0 ? T.orange : T.green }}>
                    {obs.am_alerts_active}
                  </span>
                  <span style={{ fontSize: 11, color: T.textMuted, marginLeft: 4 }}>active</span>
                </div>
                <div>
                  <span style={{ fontSize: 22, fontWeight: 800, color: T.textMuted }}>{obs.am_alerts_silenced}</span>
                  <span style={{ fontSize: 11, color: T.textMuted, marginLeft: 4 }}>silenced</span>
                </div>
              </div>
            </div>

            <div style={{ fontSize: 10, color: T.textMuted, fontFamily: T.mono }}>{obs.fetched_at}</div>
          </div>
        ) : (
          <div style={{ textAlign: "center", padding: "20px 0", color: T.textMuted, fontSize: 12 }}>Loading...</div>
        )}
      </Card>
    </div>
  );
}

// ── Attack Lab Panel ────────────────────────────────────────────────────────

function AttackLabPanel() {
  const [attacks, setAttacks] = useState<AttackDef[]>([]);
  const [result, setResult] = useState<AttackResult | null>(null);
  const [running, setRunning] = useState(false);
  const [firingIdx, setFiringIdx] = useState<number | null>(null);

  useEffect(() => {
    listAttacks().then((a) => { if (a) setAttacks(a); });
  }, []);

  const fire = useCallback(async (idx: number) => {
    setRunning(true);
    setFiringIdx(idx);
    setResult(null);
    const r = await runAttack(idx);
    if (r) setResult(r);
    setRunning(false);
    setFiringIdx(null);
  }, []);

  const severityColor = (s: string) => {
    switch (s) {
      case "critical": return { color: T.red, bg: T.redBg, border: `${T.red}30` };
      case "high":     return { color: T.orange, bg: T.orangeBg, border: `${T.orange}30` };
      case "medium":   return { color: T.accent, bg: T.accentBg, border: `${T.accent}30` };
      default:         return { color: T.textMuted, bg: "rgba(120,128,145,0.08)", border: "rgba(120,128,145,0.2)" };
    }
  };

  return (
    <div style={{ padding: "0 16px 20px" }}>
      <Card>
        <div style={{ fontSize: 18, fontWeight: 700, color: T.textBright, marginBottom: 4 }}>Attack Lab</div>
        <div style={{ fontSize: 11, color: T.textMuted, marginBottom: 12 }}>MITRE ATT&CK simulation scenarios</div>

        {attacks.length === 0 && (
          <div style={{ padding: "20px 0", textAlign: "center" }}>
            <span style={{ fontSize: 12, color: T.textMuted }}>Loading attacks...</span>
          </div>
        )}

        <div style={{ display: "flex", flexDirection: "column", gap: 8 }}>
          {attacks.map((a, i) => {
            const sc = severityColor(a.severity);
            const isFiring = firingIdx === i;
            return (
              <div key={a.rule_id} style={{
                padding: "12px 14px", borderRadius: T.radius,
                background: sc.bg, border: `1px solid ${sc.border}`,
                transition: T.transition,
              }}>
                <div style={{ display: "flex", alignItems: "center", gap: 10 }}>
                  <button
                    onClick={() => fire(i)} disabled={running}
                    style={{
                      width: 48, height: 48, borderRadius: T.radiusSm,
                      border: `1px solid ${sc.border}`,
                      background: isFiring
                        ? `linear-gradient(135deg, ${sc.color}30, ${sc.color}15)`
                        : T.navBg,
                      color: sc.color, cursor: running ? "not-allowed" : "pointer",
                      fontSize: 11, fontWeight: 800, fontFamily: T.mono,
                      letterSpacing: "0.05em", transition: T.transition,
                      display: "flex", alignItems: "center", justifyContent: "center",
                      flexShrink: 0,
                      opacity: running && !isFiring ? 0.3 : 1,
                      animation: isFiring ? "pulse-glow 1s infinite" : "none",
                    }}
                  >
                    {isFiring ? <IcLoader size={16} /> : "FIRE"}
                  </button>

                  <div style={{ flex: 1, minWidth: 0 }}>
                    <div style={{
                      fontSize: 12, fontWeight: 700, color: T.textBright,
                      whiteSpace: "nowrap", overflow: "hidden", textOverflow: "ellipsis",
                    }}>
                      {a.name}
                    </div>
                    <div style={{ fontSize: 10, color: T.textMuted, marginTop: 3, display: "flex", gap: 8, flexWrap: "wrap" }}>
                      <PillLabel text={a.mitre} color={sc.color} />
                      <span>{a.events} events</span>
                      <span>{a.description}</span>
                    </div>
                  </div>

                  <PillLabel text={a.severity} color={sc.color} />
                </div>
              </div>
            );
          })}
        </div>

        {result && (
          <div style={{
            marginTop: 12, padding: "12px 14px", borderRadius: T.radius,
            background: result.success
              ? `linear-gradient(135deg, ${T.greenBg}, rgba(90,200,140,0.04))`
              : `linear-gradient(135deg, ${T.redBg}, rgba(235,75,85,0.04))`,
            border: `1px solid ${result.success ? `${T.green}30` : `${T.red}30`}`,
            display: "flex", alignItems: "center", gap: 10,
          }}>
            <span style={{ display: "flex", color: result.success ? T.green : T.red }}>
              {result.success ? <IcCheck size={18} /> : <IcX size={18} />}
            </span>
            <div>
              <div style={{ fontSize: 12, fontWeight: 600, color: result.success ? T.green : T.red }}>{result.attack_name}</div>
              <div style={{ fontSize: 11, color: T.textMuted }}>
                {result.events_sent} events sent
                {!result.success && result.error && ` \u2014 ${result.error}`}
              </div>
            </div>
          </div>
        )}
      </Card>

      <style>{`
        @keyframes pulse-glow {
          0%, 100% { box-shadow: 0 0 4px rgba(235,75,85,0.2); }
          50%      { box-shadow: 0 0 16px rgba(235,75,85,0.5); }
        }
      `}</style>
    </div>
  );
}

// ── Settings Panel ──────────────────────────────────────────────────────────

function SettingsPanel() {
  const [settings, setSettings] = useState<AppSettings | null>(null);
  const [saved, setSaved] = useState(false);

  useEffect(() => {
    getSettings().then(s => { if (s) setSettings(s); });
  }, []);

  const update = useCallback(<K extends keyof AppSettings>(key: K, value: AppSettings[K]) => {
    setSettings(prev => prev ? { ...prev, [key]: value } : prev);
  }, []);

  const save = useCallback(async () => {
    if (!settings) return;
    const ok = await saveSettings(settings);
    if (ok) { setSaved(true); setTimeout(() => setSaved(false), 2000); }
  }, [settings]);

  if (!settings) return null;

  const inputStyle: React.CSSProperties = {
    width: "100%", padding: "7px 10px", borderRadius: T.radiusSm,
    border: `1px solid ${T.border}`, background: T.bgInput,
    color: T.text, fontSize: 12, fontFamily: T.mono,
    outline: "none", transition: T.transition,
  };

  const sectionTitle = (label: string, color: string) => (
    <div style={{ fontSize: 12, fontWeight: 700, color, marginBottom: 8, paddingBottom: 4, borderBottom: `1px solid ${T.borderLight}` }}>{label}</div>
  );

  return (
    <div style={{ padding: "0 16px 20px" }}>
      <Card>
        <div style={{ fontSize: 18, fontWeight: 700, color: T.textBright, marginBottom: 12 }}>Config</div>

        {/* Connection */}
        <div style={{
          padding: "12px 14px", borderRadius: T.radiusSm, marginBottom: 10,
          background: T.bgInput, border: `1px solid ${T.borderLight}`,
        }}>
          {sectionTitle("Connection", T.accent)}
          <div style={{ marginBottom: 10 }}>
            <FieldLabel>API URL</FieldLabel>
            <input style={inputStyle} value={settings.api_base} onChange={e => update("api_base", e.target.value)} />
          </div>
          <div>
            <FieldLabel>Detection Engine URL</FieldLabel>
            <input style={inputStyle} value={settings.detection_engine_url} onChange={e => update("detection_engine_url", e.target.value)} />
          </div>
        </div>

        {/* Appearance */}
        <div style={{
          padding: "12px 14px", borderRadius: T.radiusSm, marginBottom: 10,
          background: T.bgInput, border: `1px solid ${T.borderLight}`,
        }}>
          {sectionTitle("Appearance", "rgb(167,139,250)")}
          <Toggle label="Compact mode" checked={settings.compact_mode} onChange={v => update("compact_mode", v)} />
          <div style={{ marginTop: 8 }}>
            <FieldLabel>Theme</FieldLabel>
            <select value={settings.theme_mode} onChange={e => update("theme_mode", e.target.value)} style={{ ...inputStyle, cursor: "pointer" }}>
              <option value="dark">Dark</option>
              <option value="light">Light</option>
              <option value="system">System</option>
            </select>
          </div>
        </div>

        {/* Behavior */}
        <div style={{
          padding: "12px 14px", borderRadius: T.radiusSm, marginBottom: 10,
          background: T.bgInput, border: `1px solid ${T.borderLight}`,
        }}>
          {sectionTitle("Behavior", T.green)}
          <Toggle label="Auto refresh" checked={settings.auto_refresh_enabled} onChange={v => update("auto_refresh_enabled", v)} />
          {settings.auto_refresh_enabled && (
            <div style={{ marginTop: 8 }}>
              <FieldLabel>Refresh interval: {settings.auto_refresh_interval_sec}s</FieldLabel>
              <input type="range" min={5} max={120} step={5}
                value={settings.auto_refresh_interval_sec}
                onChange={e => update("auto_refresh_interval_sec", Number(e.target.value))}
                style={{ width: "100%", accentColor: T.accent }}
              />
            </div>
          )}
        </div>

        {/* Identity */}
        <div style={{
          padding: "12px 14px", borderRadius: T.radiusSm, marginBottom: 10,
          background: T.bgInput, border: `1px solid ${T.borderLight}`,
        }}>
          {sectionTitle("Identity", T.orange)}
          <div style={{ marginBottom: 10 }}>
            <FieldLabel>Username</FieldLabel>
            <input style={inputStyle} value={settings.whoami} onChange={e => update("whoami", e.target.value)} />
          </div>
          <div>
            <FieldLabel>Role</FieldLabel>
            <select value={settings.role} onChange={e => update("role", e.target.value)} style={{ ...inputStyle, cursor: "pointer" }}>
              <option value="analyst">Analyst</option>
              <option value="senior">Senior Analyst</option>
              <option value="manager">SOC Manager</option>
            </select>
          </div>
        </div>

        <ActionBtn
          label={saved ? "Saved" : "Save Settings"}
          icon={saved ? <IcCheck size={14} /> : <IcPlay size={13} />}
          variant={saved ? "green" : "accent"}
          onClick={save}
          style={{ width: "100%", justifyContent: "center", padding: "10px" }}
        />
      </Card>
    </div>
  );
}

// ── Main component ──────────────────────────────────────────────────────────

export default function TauriPanel() {
  const [open, setOpen] = useState(false);
  const [inTauri, setInTauri] = useState(false);
  const [section, setSection] = useState<SectionId>("overview");

  useEffect(() => {
    setInTauri(isTauriSync());
  }, []);

  if (!inTauri) return null;

  const SIDEBAR_W = 190;
  const PANEL_W = 560;

  return (
    <>
      <style>{`
        @keyframes tauri-panel-spin {
          to { transform: rotate(360deg); }
        }
      `}</style>
      {/* Floating toggle button */}
      <button
        onClick={() => setOpen(!open)}
        style={{
          position: "fixed",
          right: open ? PANEL_W + 12 : 16,
          top: 14,
          zIndex: 10000,
          width: 42, height: 42,
          borderRadius: 12,
          border: `1px solid ${T.border}`,
          background: open ? T.bgCard : "rgba(14,18,24,0.9)",
          backdropFilter: "blur(12px)",
          color: T.accent,
          cursor: "pointer",
          fontSize: 18,
          lineHeight: 1,
          display: "flex", alignItems: "center", justifyContent: "center",
          transition: "right 0.3s cubic-bezier(0.4, 0, 0.2, 1), background 0.2s ease",
          boxShadow: "0 2px 16px rgba(0,0,0,0.4)",
        }}
        title="SIEM Desktop"
      >
        {open ? <IcX size={18} /> : <IcSettings size={18} />}
      </button>

      {/* Slide-out panel */}
      <div style={{
        position: "fixed", right: 0, top: 0, bottom: 0,
        width: PANEL_W,
        background: T.bg,
        borderLeft: `1px solid ${T.sidebarBorder}`,
        zIndex: 9999,
        display: "flex", flexDirection: "row",
        fontFamily: T.font,
        color: T.text,
        fontSize: 13,
        transform: open ? "translateX(0)" : "translateX(100%)",
        transition: "transform 0.3s cubic-bezier(0.4, 0, 0.2, 1)",
      }}>
        {/* ── Sidebar ── */}
        <div style={{
          width: SIDEBAR_W, flexShrink: 0,
          background: T.sidebarBg,
          borderRight: `1px solid ${T.sidebarBorder}`,
          display: "flex", flexDirection: "column",
          overflowY: "auto",
        }}>
          {/* Sidebar header */}
          <div style={{
            padding: "18px 14px 12px",
            borderBottom: `1px solid ${T.sidebarBorder}`,
            display: "flex", alignItems: "center", justifyContent: "space-between",
          }}>
            <div style={{ display: "flex", alignItems: "flex-start", gap: 8, minWidth: 0 }}>
              <span style={{ color: T.accent, marginTop: 2, flexShrink: 0 }}><IcSettings size={18} /></span>
              <div style={{ minWidth: 0 }}>
                <div style={{ fontSize: 17, fontWeight: 800, color: T.textWhite, lineHeight: 1.2 }}>SIEM Desktop</div>
                <div style={{ fontSize: 12, color: T.accent, fontWeight: 500 }}>Tauri + React</div>
              </div>
            </div>
            <button
              onClick={() => setSection("settings")}
              style={{
                width: 30, height: 30, borderRadius: T.radiusSm,
                border: `1px solid ${section === "settings" ? T.accent : T.border}`,
                background: section === "settings" ? T.accentBg : "transparent",
                color: section === "settings" ? T.accent : T.textMuted,
                cursor: "pointer", fontSize: 14,
                display: "flex", alignItems: "center", justifyContent: "center",
                transition: T.transition,
              }}
              title="Config"
            >
              <IcSettings size={16} />
            </button>
          </div>

          {/* Section label */}
          <div style={{
            padding: "10px 14px 4px",
            fontSize: 10, fontWeight: 600, color: T.textMuted,
            textTransform: "uppercase", letterSpacing: "0.08em",
          }}>
            Разделы
          </div>

          {/* Nav buttons */}
          {NAV_ITEMS.filter(n => n.id !== "settings").map(nav => {
            const isActive = section === nav.id;
            return (
              <button
                key={nav.id}
                onClick={() => setSection(nav.id)}
                style={{
                  display: "flex", flexDirection: "column", alignItems: "flex-start",
                  padding: "10px 14px", margin: "2px 8px", borderRadius: T.radius,
                  border: isActive ? `1px solid ${T.navActiveBorder}` : "1px solid transparent",
                  background: isActive ? T.navActive : "transparent",
                  cursor: "pointer", width: "calc(100% - 16px)",
                  transition: T.transition, textAlign: "left",
                }}
                onMouseEnter={e => { if (!isActive) (e.currentTarget.style.background = T.navHover); }}
                onMouseLeave={e => { if (!isActive) (e.currentTarget.style.background = "transparent"); }}
              >
                <div style={{ display: "flex", alignItems: "center", gap: 8 }}>
                  <span style={{ display: "flex", alignItems: "center", justifyContent: "center", width: 20, color: isActive ? T.accent : T.textMuted }}>{nav.icon}</span>
                  <span style={{ fontSize: 13, fontWeight: isActive ? 700 : 500, color: isActive ? T.textBright : T.text }}>{nav.label}</span>
                </div>
                <span style={{ fontSize: 10, color: T.textMuted, marginLeft: 26, marginTop: 1 }}>{nav.subtitle}</span>
              </button>
            );
          })}

          {/* Version */}
          <div style={{ marginTop: "auto", padding: "12px 14px", borderTop: `1px solid ${T.sidebarBorder}` }}>
            <div style={{ fontSize: 9, color: T.textMuted, fontFamily: T.mono }}>v0.3 · Tauri + React</div>
          </div>
        </div>

        {/* ── Content area ── */}
        <div style={{
          flex: 1, display: "flex", flexDirection: "column", overflow: "hidden",
        }}>
          {/* Toolbar */}
          <div style={{
            padding: "14px 16px 10px",
            borderBottom: `1px solid ${T.borderLight}`,
            background: "linear-gradient(180deg, rgba(120,190,255,0.04) 0%, transparent 100%)",
            display: "flex", alignItems: "center", gap: 8, flexShrink: 0,
          }}>
            <span style={{ display: "inline-flex", alignItems: "center", gap: 6, fontSize: 14, fontWeight: 800, color: T.textWhite }}>
              <span style={{ color: T.accent, display: "flex" }}><IcSettings size={16} /></span>
              SIEM Desktop
            </span>
            <span style={{ fontSize: 12, color: T.textMuted }}>
              / {NAV_ITEMS.find(n => n.id === section)?.label}
            </span>
          </div>

          {/* Scrollable content */}
          <div style={{
            flex: 1, overflowY: "auto", paddingTop: 12,
            scrollbarWidth: "thin", scrollbarColor: `${T.border} transparent`,
          }}>
            {section === "overview" && <OverviewPanel onNavigate={setSection} />}
            {section === "stack"    && <StackControlPanel />}
            {section === "obs"      && <ObservabilityPanel />}
            {section === "attacks"  && <AttackLabPanel />}
            {section === "settings" && <SettingsPanel />}
          </div>
        </div>
      </div>
    </>
  );
}
