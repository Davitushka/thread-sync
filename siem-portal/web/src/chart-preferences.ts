/** Включённые анимации ECharts (тяжелее в WebView; по умолчанию выкл.). */
export const CHART_ANIMATIONS_STORAGE_KEY = "suite_chart_animations";

export function readChartAnimationsEnabled(): boolean {
  if (typeof localStorage === "undefined") return false;
  try {
    return localStorage.getItem(CHART_ANIMATIONS_STORAGE_KEY) === "1";
  } catch {
    return false;
  }
}

export function writeChartAnimationsEnabled(enabled: boolean): void {
  if (typeof localStorage === "undefined") return;
  try {
    if (enabled) {
      localStorage.setItem(CHART_ANIMATIONS_STORAGE_KEY, "1");
    } else {
      localStorage.removeItem(CHART_ANIMATIONS_STORAGE_KEY);
    }
    window.dispatchEvent(new Event("suite-chart-animations-changed"));
  } catch {
    /* ignore quota */
  }
}
