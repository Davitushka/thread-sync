export const PERF_OVERLAY_STORAGE_KEY = "suite_perf_overlay";

export function readPerfOverlayEnabled(): boolean {
  if (typeof localStorage === "undefined") return false;
  try {
    return localStorage.getItem(PERF_OVERLAY_STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

export function writePerfOverlayEnabled(enabled: boolean): void {
  if (typeof localStorage === "undefined") return;
  try {
    if (enabled) localStorage.setItem(PERF_OVERLAY_STORAGE_KEY, "1");
    else localStorage.removeItem(PERF_OVERLAY_STORAGE_KEY);
    window.dispatchEvent(new Event("suite-perf-overlay-changed"));
  } catch {
    // ignore quota errors
  }
}
