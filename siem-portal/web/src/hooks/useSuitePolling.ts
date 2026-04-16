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

    /** Пока пользователь крутит колесо/тач-скролл, не дергаем тяжёлые refetch — меньше рывков в Operator WebView. */
    let scrollQuietTimer: number | null = null;
    const scrollBusy = { current: false };

    const markScrolling = () => {
      scrollBusy.current = true;
      if (scrollQuietTimer != null) window.clearTimeout(scrollQuietTimer);
      scrollQuietTimer = window.setTimeout(() => {
        scrollBusy.current = false;
        scrollQuietTimer = null;
      }, 220);
    };

    const run = () => {
      if (document.visibilityState !== "visible") return;
      if (scrollBusy.current) return;
      cb.current();
    };

    const id = window.setInterval(run, intervalSec * 1000);

    const onVis = () => {
      if (document.visibilityState === "visible") {
        if (!scrollBusy.current) cb.current();
      }
    };
    document.addEventListener("visibilitychange", onVis);
    window.addEventListener("wheel", markScrolling, { passive: true });
    window.addEventListener("touchmove", markScrolling, { passive: true });

    return () => {
      window.clearInterval(id);
      if (scrollQuietTimer != null) window.clearTimeout(scrollQuietTimer);
      document.removeEventListener("visibilitychange", onVis);
      window.removeEventListener("wheel", markScrolling);
      window.removeEventListener("touchmove", markScrolling);
    };
  }, [intervalSec]);
}
