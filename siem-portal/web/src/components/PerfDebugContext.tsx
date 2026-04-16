import { createContext, useCallback, useContext, useEffect, useMemo, useState, type ReactNode } from "react";
import { PERF_OVERLAY_STORAGE_KEY, readPerfOverlayEnabled, writePerfOverlayEnabled } from "../perf-debug";

type PerfDebugContextValue = {
  perfOverlayEnabled: boolean;
  setPerfOverlayEnabled: (next: boolean) => void;
};

const PerfDebugContext = createContext<PerfDebugContextValue | null>(null);

export function PerfDebugProvider({ children }: { children: ReactNode }) {
  const [perfOverlayEnabled, setPerfOverlayEnabledState] = useState(() => readPerfOverlayEnabled());

  useEffect(() => {
    const sync = () => setPerfOverlayEnabledState(readPerfOverlayEnabled());
    const onStorage = (e: StorageEvent) => {
      if (e.key === PERF_OVERLAY_STORAGE_KEY || e.key === null) sync();
    };
    window.addEventListener("suite-perf-overlay-changed", sync);
    window.addEventListener("storage", onStorage);
    return () => {
      window.removeEventListener("suite-perf-overlay-changed", sync);
      window.removeEventListener("storage", onStorage);
    };
  }, []);

  const setPerfOverlayEnabled = useCallback((next: boolean) => {
    writePerfOverlayEnabled(next);
    setPerfOverlayEnabledState(next);
  }, []);

  const value = useMemo(
    () => ({ perfOverlayEnabled, setPerfOverlayEnabled }),
    [perfOverlayEnabled, setPerfOverlayEnabled]
  );

  return <PerfDebugContext.Provider value={value}>{children}</PerfDebugContext.Provider>;
}

export function usePerfDebug() {
  const v = useContext(PerfDebugContext);
  if (!v) throw new Error("usePerfDebug must be used within PerfDebugProvider");
  return v;
}
