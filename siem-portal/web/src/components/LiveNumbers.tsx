import { animate, useMotionValue } from "framer-motion";
import { useEffect, useState } from "react";
import { formatCompact, formatPercent } from "../dashboard-utils";

type LiveCompactProps = {
  value: number | null | undefined;
  duration?: number;
  className?: string;
};

export function LiveCompactNumber({ value, duration = 0.65, className }: LiveCompactProps) {
  const mv = useMotionValue(0);
  const [text, setText] = useState("—");

  useEffect(() => {
    if (value == null || Number.isNaN(value)) {
      mv.set(0);
      setText("—");
      return;
    }
    const end = Math.round(value);
    const start = Math.round(mv.get());
    if (start === end) {
      mv.set(end);
      setText(formatCompact(end));
      return;
    }
    const from = start;
    mv.set(from);
    const controls = animate(from, end, {
      duration,
      ease: [0.22, 1, 0.36, 1],
      onUpdate: (latest) => {
        mv.set(latest);
        setText(formatCompact(Math.round(latest)));
      },
    });
    return () => controls.stop();
  }, [value, duration, mv]);

  return <span className={className}>{text}</span>;
}

type LivePercentProps = {
  value: number | null | undefined;
  duration?: number;
  className?: string;
};

export function LivePercentNumber({ value, duration = 0.65, className }: LivePercentProps) {
  const mv = useMotionValue(0);
  const [text, setText] = useState("—");

  useEffect(() => {
    if (value == null || Number.isNaN(value)) {
      mv.set(0);
      setText("—");
      return;
    }
    const end = value;
    const start = mv.get();
    if (Number.isFinite(start) && Math.abs(start - end) < 1e-6) {
      mv.set(end);
      setText(formatPercent(end));
      return;
    }
    const from = Number.isFinite(start) && Math.abs(start - end) > 1e-9 ? start : 0;
    mv.set(from);
    const controls = animate(from, end, {
      duration,
      ease: [0.22, 1, 0.36, 1],
      onUpdate: (latest) => {
        mv.set(latest);
        setText(formatPercent(latest));
      },
    });
    return () => controls.stop();
  }, [value, duration, mv]);

  return <span className={className}>{text}</span>;
}
