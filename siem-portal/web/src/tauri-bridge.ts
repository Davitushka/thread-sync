/**
 * Tauri bridge — provides typed access to siem-desktop Rust commands.
 *
 * Three transport modes:
 * 1. Direct invoke() — when window.__TAURI__ exists (Tauri asset protocol)
 * 2. postMessage proxy — when running in an iframe with ?tauri=1
 *    (parent frame retains __TAURI__ and proxies invoke calls)
 * 3. Standalone browser — all functions return null
 *
 * IMPORTANT: Tauri 2 uses Rust parameter names as JSON keys,
 * so we must send snake_case keys (api_base, not apiBase).
 */

// Lazy reference to the real invoke — loaded on first use if available
let _directInvoke: ((cmd: string, args?: Record<string, unknown>) => Promise<any>) | null = null;
let _invokeLoaded = false;

async function loadInvoke(): Promise<typeof _directInvoke> {
  if (_invokeLoaded) return _directInvoke;
  _invokeLoaded = true;
  try {
    const mod = await import("@tauri-apps/api/core");
    _directInvoke = mod.invoke;
  } catch {
    _directInvoke = null;
  }
  return _directInvoke;
}

// ── Types ────────────────────────────────────────────────────────────────────

export interface ServiceStatus {
  name: string;
  url: string;
  healthy: boolean;
}

export interface StackStatus {
  services: ServiceStatus[];
}

export interface StackServiceStatus {
  service: string;
  status: string;
  detail: string;
}

export interface PortalStackStatus {
  services: StackServiceStatus[];
}

export interface ObsSnapshot {
  fetched_at: string;
  prom_total_targets: number;
  prom_up_targets: number;
  prom_version: string;
  am_alerts_active: number;
  am_alerts_silenced: number;
}

export interface AppSettings {
  api_base: string;
  detection_engine_url: string;
  auto_refresh_enabled: boolean;
  auto_refresh_interval_sec: number;
  theme_mode: string;
  compact_mode: boolean;
  whoami: string;
  role: string;
}

export interface AttackDef {
  name: string;
  rule_id: string;
  severity: string;
  mitre: string;
  events: number;
  description: string;
}

export interface AttackResult {
  attack_name: string;
  events_sent: number;
  success: boolean;
  error: string | null;
}

// ── Transport detection ─────────────────────────────────────────────────────

type TransportMode = "direct" | "postMessage" | "none";
let _transport: TransportMode | null = null;

function detectTransport(): TransportMode {
  // 1. Direct Tauri context — __TAURI__ exists on this origin
  if (!!(window as any).__TAURI__) return "direct";
  // 2. Iframe with ?tauri=1 — parent frame proxies invoke calls
  if (typeof URLSearchParams !== "undefined") {
    const p = new URLSearchParams(window.location.search);
    if (p.get("tauri") === "1" && window.parent !== window) return "postMessage";
  }
  // 3. Standalone browser
  return "none";
}

/** Returns true when running inside siem-desktop (Tauri WebView). */
export function isTauriSync(): boolean {
  if (_transport === null) _transport = detectTransport();
  return _transport !== "none";
}

// ── postMessage IPC ─────────────────────────────────────────────────────────

let _ipcId = 0;
const _pending = new Map<number, { resolve: (v: any) => void; reject: (e: any) => void }>();

window.addEventListener("message", (event) => {
  if (!event.data || event.data.__siem_ipc !== true) return;
  if (event.data.type !== "response") return;
  const { id, result, error } = event.data;
  const entry = _pending.get(id);
  if (!entry) return;
  _pending.delete(id);
  if (error) entry.reject(new Error(error));
  else entry.resolve(result);
});

function postMessageInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T> {
  return new Promise((resolve, reject) => {
    const id = ++_ipcId;
    _pending.set(id, { resolve, reject });
    window.parent.postMessage({ __siem_ipc: true, type: "request", id, cmd, args: args || {} }, "*");
    // Timeout after 30s
    setTimeout(() => {
      if (_pending.has(id)) {
        _pending.delete(id);
        reject(new Error(`IPC timeout for ${cmd}`));
      }
    }, 30000);
  });
}

// ── Internal invoke wrapper ─────────────────────────────────────────────────

async function safeInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T | null> {
  if (_transport === null) _transport = detectTransport();
  if (_transport === "none") return null;

  try {
    if (_transport === "direct") {
      const inv = await loadInvoke();
      if (inv) return (await inv(cmd, args)) as T;
    }
    if (_transport === "postMessage") {
      return (await postMessageInvoke<T>(cmd, args)) as T;
    }
    return null;
  } catch (e) {
    console.warn(`[tauri-bridge] ${cmd} failed:`, e);
    return null;
  }
}

// ── Public API ───────────────────────────────────────────────────────────────
// NOTE: all parameter keys use snake_case to match Rust parameter names

export async function checkStackStatus(): Promise<StackStatus | null> {
  return safeInvoke<StackStatus>("check_stack_status");
}

export async function fetchPortalStackStatus(apiBase: string): Promise<PortalStackStatus | null> {
  return safeInvoke<PortalStackStatus>("fetch_portal_stack_status", { api_base: apiBase });
}

export async function fetchObservabilitySnapshot(apiBase: string): Promise<ObsSnapshot | null> {
  return safeInvoke<ObsSnapshot>("fetch_observability_snapshot", { api_base: apiBase });
}

export async function dockerComposeAction(action: "start" | "stop" | "restart" | "status"): Promise<string | null> {
  return safeInvoke<string>("docker_compose_action", { action });
}

export async function getDockerOutput(): Promise<string | null> {
  return safeInvoke<string>("get_docker_output");
}

export async function getSettings(): Promise<AppSettings | null> {
  return safeInvoke<AppSettings>("get_settings");
}

export async function saveSettings(settings: AppSettings): Promise<boolean> {
  const result = await safeInvoke<null>("save_settings", { settings });
  return result !== null;
}

export async function listAttacks(): Promise<AttackDef[] | null> {
  return safeInvoke<AttackDef[]>("list_attacks");
}

export async function runAttack(attackIdx: number): Promise<AttackResult | null> {
  return safeInvoke<AttackResult>("run_attack", { attack_idx: attackIdx });
}

export async function openExternal(url: string): Promise<boolean> {
  const result = await safeInvoke<null>("open_external", { url });
  return result !== null;
}

export async function getPortalUrl(): Promise<string | null> {
  return safeInvoke<string>("get_portal_url");
}

export async function getAppVersion(): Promise<string | null> {
  return safeInvoke<string>("get_app_version");
}

export async function getEnvUrl(key: string): Promise<string | null> {
  return safeInvoke<string>("get_env_url", { key });
}
