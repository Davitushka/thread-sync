import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useState,
  type ReactNode,
} from "react";
import {
  CHART_ANIMATIONS_STORAGE_KEY,
  readChartAnimationsEnabled,
  writeChartAnimationsEnabled,
} from "../chart-preferences";

export type ChartMotionContextValue = {
  /** Полные анимации ECharts (линии, gauge и т.д.). */
  chartAnimationsEnabled: boolean;
  setChartAnimationsEnabled: (next: boolean) => void;
};

const ChartMotionContext = createContext<ChartMotionContextValue | null>(null);

export function ChartMotionProvider({ children }: { children: ReactNode }) {
  const [chartAnimationsEnabled, setChartAnimationsEnabledState] = useState(() => readChartAnimationsEnabled());

  useEffect(() => {
    const sync = () => setChartAnimationsEnabledState(readChartAnimationsEnabled());
    const onStorage = (e: StorageEvent) => {
      if (e.key === CHART_ANIMATIONS_STORAGE_KEY || e.key === null) {
        sync();
      }
    };
    window.addEventListener("suite-chart-animations-changed", sync);
    window.addEventListener("storage", onStorage);
    return () => {
      window.removeEventListener("suite-chart-animations-changed", sync);
      window.removeEventListener("storage", onStorage);
    };
  }, []);

  const setChartAnimationsEnabled = useCallback((next: boolean) => {
    writeChartAnimationsEnabled(next);
    setChartAnimationsEnabledState(next);
  }, []);

  const value = useMemo<ChartMotionContextValue>(
    () => ({ chartAnimationsEnabled, setChartAnimationsEnabled }),
    [chartAnimationsEnabled, setChartAnimationsEnabled]
  );

  return <ChartMotionContext.Provider value={value}>{children}</ChartMotionContext.Provider>;
}

export function useChartMotion(): ChartMotionContextValue {
  const v = useContext(ChartMotionContext);
  if (!v) {
    throw new Error("useChartMotion must be used within ChartMotionProvider");
  }
  return v;
}
