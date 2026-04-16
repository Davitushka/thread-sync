import CountUp from "react-countup";
import { formatCompact } from "../dashboard-utils";

type LiveCompactProps = {
  value: number | null | undefined;
  duration?: number;
  className?: string;
};

export function LiveCompactNumber({ value, duration = 0.55, className }: LiveCompactProps) {
  if (value == null || Number.isNaN(value)) {
    return <span className={className}>—</span>;
  }
  const end = Math.round(value);
  return (
    <CountUp
      className={className}
      end={end}
      duration={duration}
      preserveValue
      useEasing
      formattingFn={(n) => formatCompact(Math.round(n))}
    />
  );
}

type LivePercentProps = {
  value: number | null | undefined;
  duration?: number;
  className?: string;
};

export function LivePercentNumber({ value, duration = 0.55, className }: LivePercentProps) {
  if (value == null || Number.isNaN(value)) {
    return <span className={className}>—</span>;
  }
  return (
    <CountUp
      className={className}
      end={value}
      duration={duration}
      preserveValue
      useEasing
      decimals={2}
      decimal="."
      suffix="%"
    />
  );
}
