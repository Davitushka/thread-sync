/**
 * Tauri bridge — provides typed access to siem-desktop Rust commands.
 *
 * When the portal is loaded inside siem-desktop (Tauri WebView),
 * the @tauri-apps/api invoke() works.
 * When loaded in a regular browser, all functions return null
 * so the portal works as a standalone web app.
 */

import { invoke } from "@tauri-apps/api/core";

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

// ── Detection ────────────────────────────────────────────────────────────────

let _isTauri: boolean | null = null;

/** Returns true when running inside siem-desktop (Tauri WebView). */
export function isTauriSync(): boolean {
  if (_isTauri !== null) return _isTauri;
  _isTauri = !!(window as any).__TAURI__;
  return _isTauri;
}

// ── Internal invoke wrapper ──────────────────────────────────────────────────

async function safeInvoke<T>(cmd: string, args?: Record<string, unknown>): Promise<T | null> {
  if (!isTauriSync()) return null;
  try {
    return (await invoke(cmd, args)) as T;
  } catch (e) {
    console.warn(`[tauri-bridge] ${cmd} failed:`, e);
    return null;
  }
}

// ── Public API ───────────────────────────────────────────────────────────────

export async function checkStackStatus(): Promise<StackStatus | null> {
  return safeInvoke<StackStatus>("check_stack_status");
}

export async function fetchPortalStackStatus(apiBase: string): Promise<PortalStackStatus | null> {
  return safeInvoke<PortalStackStatus>("fetch_portal_stack_status", { apiBase });
}

export async function fetchObservabilitySnapshot(apiBase: string): Promise<ObsSnapshot | null> {
  return safeInvoke<ObsSnapshot>("fetch_observability_snapshot", { apiBase });
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
  return safeInvoke<AttackResult>("run_attack", { attackIdx });
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
