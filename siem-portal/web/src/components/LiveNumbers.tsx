import { useEffect, useRef } from "react";
import { formatCompact, formatPercent } from "../dashboard-utils";

/** Cubic-bezier easing [0.22, 1, 0.36, 1] approximated as a simple out-cubic */
function easeOutCubic(t: number): number {
  return 1 - Math.pow(1 - t, 3);
}

function animateValue(
  from: number,
  to: number,
  durationMs: number,
  onUpdate: (v: number) => void,
): () => void {
  let rafId = 0;
  const t0 = performance.now();
  const tick = () => {
    const elapsed = performance.now() - t0;
    const progress = Math.min(elapsed / durationMs, 1);
    const eased = easeOutCubic(progress);
    onUpdate(from + (to - from) * eased);
    if (progress < 1) {
      rafId = requestAnimationFrame(tick);
    }
  };
  rafId = requestAnimationFrame(tick);
  return () => cancelAnimationFrame(rafId);
}

type LiveCompactProps = {
  value: number | null | undefined;
  duration?: number;
  className?: string;
};

export function LiveCompactNumber({ value, duration = 650, className }: LiveCompactProps) {
  const spanRef = useRef<HTMLSpanElement>(null);
  const prevRef = useRef(0);

  useEffect(() => {
    if (value == null || Number.isNaN(value)) {
      prevRef.current = 0;
      if (spanRef.current) spanRef.current.textContent = "\u2014";
      return;
    }
    const end = Math.round(value);
    const start = Math.round(prevRef.current);
    if (start === end) {
      if (spanRef.current) spanRef.current.textContent = formatCompact(end);
      return;
    }
    prevRef.current = end;
    return animateValue(start, end, duration, (v) => {
      if (spanRef.current) spanRef.current.textContent = formatCompact(Math.round(v));
    });
  }, [value, duration]);

  return <span ref={spanRef} className={className}>{"\u2014"}</span>;
}

type LivePercentProps = {
  value: number | null | undefined;
  duration?: number;
  className?: string;
};

export function LivePercentNumber({ value, duration = 650, className }: LivePercentProps) {
  const spanRef = useRef<HTMLSpanElement>(null);
  const prevRef = useRef(0);

  useEffect(() => {
    if (value == null || Number.isNaN(value)) {
      prevRef.current = 0;
      if (spanRef.current) spanRef.current.textContent = "\u2014";
      return;
    }
    const end = value;
    const start = prevRef.current;
    if (Number.isFinite(start) && Math.abs(start - end) < 1e-6) {
      if (spanRef.current) spanRef.current.textContent = formatPercent(end);
      return;
    }
    const from = Number.isFinite(start) && Math.abs(start - end) > 1e-9 ? start : 0;
    prevRef.current = end;
    return animateValue(from, end, duration, (v) => {
      if (spanRef.current) spanRef.current.textContent = formatPercent(v);
    });
  }, [value, duration]);

  return <span ref={spanRef} className={className}>{"\u2014"}</span>;
}
