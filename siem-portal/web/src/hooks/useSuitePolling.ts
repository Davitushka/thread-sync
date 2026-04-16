import { useCallback, useEffect, useRef, useState } from "react";
import {
  readSuiteAutoRefreshSec,
  writeSuiteAutoRefreshSec,
  type SuiteRefreshChoice,
} from "../suite-polling";

/** Сохраняемый интервал автообновления (общий для всех дашбордов). */
export function useSuiteAutoRefreshState(): [SuiteRefreshChoice, (sec: SuiteRefreshChoice) => void] {
  const [sec, setSec] = useState<SuiteRefreshChoice>(() => readSuiteAutoRefreshSec());
  const update = useCallback((next: SuiteRefreshChoice) => {
    setSec(next);
    writeSuiteAutoRefreshSec(next);
  }, []);
  return [sec, update];
}

/**
 * Периодический вызов `callback`, только когда вкладка видима.
 * При возврате на вкладку — один раз обновляет данные.
 */
export function useVisibleInterval(callback: () => void, intervalSec: number) {
  const cb = useRef(callback);
  cb.current = callback;

  useEffect(() => {
    if (!intervalSec) return;

    const run = () => {
      if (document.visibilityState !== "visible") return;
      cb.current();
    };

    const id = window.setInterval(run, intervalSec * 1000);

    const onVis = () => {
      if (document.visibilityState === "visible") {
        cb.current();
      }
    };
    document.addEventListener("visibilitychange", onVis);

    return () => {
      window.clearInterval(id);
      document.removeEventListener("visibilitychange", onVis);
    };
  }, [intervalSec]);
}
