/** Единый список интервалов автообновления (секунды), синхронизирован с DashboardToolbar. */
export const SUITE_REFRESH_CHOICES = [0, 15, 30, 60, 300] as const;
export type SuiteRefreshChoice = (typeof SUITE_REFRESH_CHOICES)[number];

export const SUITE_DEFAULT_REFRESH_SEC: SuiteRefreshChoice = 30;
export const SUITE_REFRESH_STORAGE_KEY = "suite_auto_refresh_sec";

const CHOICE_SET = new Set<number>(SUITE_REFRESH_CHOICES);

export function readSuiteAutoRefreshSec(): SuiteRefreshChoice {
  if (typeof localStorage === "undefined") return SUITE_DEFAULT_REFRESH_SEC;
  try {
    const raw = localStorage.getItem(SUITE_REFRESH_STORAGE_KEY);
    if (raw == null) return SUITE_DEFAULT_REFRESH_SEC;
    const n = Number(raw);
    return CHOICE_SET.has(n) ? (n as SuiteRefreshChoice) : SUITE_DEFAULT_REFRESH_SEC;
  } catch {
    return SUITE_DEFAULT_REFRESH_SEC;
  }
}

export function writeSuiteAutoRefreshSec(sec: number): void {
  if (typeof localStorage === "undefined") return;
  if (!CHOICE_SET.has(sec)) return;
  try {
    localStorage.setItem(SUITE_REFRESH_STORAGE_KEY, String(sec));
  } catch {
    /* ignore quota */
  }
}

export function suiteRefreshSelectOptions(): Array<{ value: SuiteRefreshChoice; label: string }> {
  const labels: Record<SuiteRefreshChoice, string> = {
    0: "manual",
    15: "15s",
    30: "30s",
    60: "1m",
    300: "5m",
  };
  return SUITE_REFRESH_CHOICES.map((value) => ({ value, label: labels[value] }));
}
