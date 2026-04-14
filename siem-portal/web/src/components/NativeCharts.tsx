type Point = { x: string; y: number };
type MultiPoint = { x: string; [key: string]: string | number };

const CHART_WIDTH = 100;
const CHART_HEIGHT = 46;
const GRIDLINES = [5, 17, 29, 41, 45];
const GAUGE_START_ANGLE = -120;
const GAUGE_SWEEP_ANGLE = 240;

function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max);
}

function normalizeSingleCoords(points: Point[], width: number, height: number, max: number) {
  if (!points.length) return [];
  return points.map((point, idx) => {
    const x = (idx / Math.max(points.length - 1, 1)) * width;
    const y = height - (point.y / max) * height;
    return { x, y };
  });
}

function normalizeSingle(points: Point[], width: number, height: number, max: number) {
  return normalizeSingleCoords(points, width, height, max)
    .map((point) => `${point.x.toFixed(2)},${point.y.toFixed(2)}`)
    .join(" ");
}

function singleAreaPath(points: Point[], width: number, height: number, max: number) {
  const coords = normalizeSingleCoords(points, width, height, max);
  if (!coords.length) return "";
  const first = coords[0];
  const last = coords[coords.length - 1];
  const path = coords.map((point) => `${point.x.toFixed(2)} ${point.y.toFixed(2)}`).join(" L ");
  return `M ${first.x.toFixed(2)} ${height.toFixed(2)} L ${path} L ${last.x.toFixed(2)} ${height.toFixed(2)} Z`;
}

function normalizeMulti(points: MultiPoint[], key: string, width: number, height: number, max: number) {
  if (!points.length) return "";
  return points
    .map((point, idx) => {
      const value = Number(point[key] ?? 0);
      const x = (idx / Math.max(points.length - 1, 1)) * width;
      const y = height - (value / max) * height;
      return `${x.toFixed(2)},${y.toFixed(2)}`;
    })
    .join(" ");
}

function gridlines(width: number, height: number) {
  return GRIDLINES.map((y) => (
    <polyline key={`${width}-${height}-${y}`} className="sparkline-gridline" points={`0,${y} ${width},${y}`} />
  ));
}

function polarToCartesian(cx: number, cy: number, radius: number, angleInDegrees: number) {
  const angleInRadians = ((angleInDegrees - 90) * Math.PI) / 180;
  return {
    x: cx + radius * Math.cos(angleInRadians),
    y: cy + radius * Math.sin(angleInRadians),
  };
}

function describeArc(cx: number, cy: number, radius: number, startAngle: number, endAngle: number) {
  const start = polarToCartesian(cx, cy, radius, endAngle);
  const end = polarToCartesian(cx, cy, radius, startAngle);
  const largeArcFlag = endAngle - startAngle <= 180 ? "0" : "1";
  return `M ${start.x.toFixed(2)} ${start.y.toFixed(2)} A ${radius} ${radius} 0 ${largeArcFlag} 0 ${end.x.toFixed(2)} ${end.y.toFixed(2)}`;
}

export function NativeLineChart({
  title,
  points,
  color,
  filled = false,
  fillOpacity = 0.16,
  fillColor,
}: {
  title: string;
  points: Point[];
  color: string;
  filled?: boolean;
  fillOpacity?: number;
  fillColor?: string;
}) {
  const max = Math.max(...points.map((point) => point.y), 1);
  const polyline = normalizeSingle(points, CHART_WIDTH, CHART_HEIGHT, max);
  const areaPath = filled ? singleAreaPath(points, CHART_WIDTH, CHART_HEIGHT, max) : "";
  return (
    <div className="native-chart-shell">
      <svg className="native-chart-svg" viewBox={`0 0 ${CHART_WIDTH} ${CHART_HEIGHT}`} role="img" aria-label={title}>
        {gridlines(CHART_WIDTH, CHART_HEIGHT)}
        {areaPath ? <path d={areaPath} fill={fillColor || color} fillOpacity={fillOpacity} /> : null}
        {polyline ? <polyline points={polyline} fill="none" stroke={color} strokeWidth="2.2" strokeLinejoin="round" /> : null}
      </svg>
    </div>
  );
}

export function NativeMultiLineChart({
  title,
  points,
  series,
}: {
  title: string;
  points: MultiPoint[];
  series: Array<{ key: string; color: string; label: string }>;
}) {
  const max = Math.max(
    1,
    ...points.flatMap((point) => series.map((item) => Number(point[item.key] ?? 0)))
  );
  return (
    <div className="native-chart-shell">
      <svg className="native-chart-svg" viewBox={`0 0 ${CHART_WIDTH} ${CHART_HEIGHT}`} role="img" aria-label={title}>
        {gridlines(CHART_WIDTH, CHART_HEIGHT)}
        {series.map((item) => {
          const polyline = normalizeMulti(points, item.key, CHART_WIDTH, CHART_HEIGHT, max);
          return polyline ? (
            <polyline
              key={item.key}
              points={polyline}
              fill="none"
              stroke={item.color}
              strokeWidth="2.2"
              strokeLinejoin="round"
            />
          ) : null;
        })}
      </svg>
      <div className="native-chart-legend">
        {series.map((item) => (
          <span key={item.key}>
            <i style={{ background: item.color }} />
            {item.label}
          </span>
        ))}
      </div>
    </div>
  );
}

export function NativeGaugeChart({
  title,
  value,
  min = 0,
  max = 100,
  unit = "%",
  detail,
  formatter,
  thresholds,
}: {
  title: string;
  value: number | null | undefined;
  min?: number;
  max?: number;
  unit?: string;
  detail?: string;
  formatter?: (value: number) => string;
  thresholds?: Array<{ value: number; color: string }>;
}) {
  const range = Math.max(max - min, 1);
  const normalizedThresholds =
    thresholds?.length
      ? [...thresholds].sort((left, right) => left.value - right.value)
      : [
          { value: min + range * 0.7, color: "#7be37c" },
          { value: min + range * 0.9, color: "#f0c15d" },
          { value: max, color: "#f85149" },
        ];
  const hasValue = value != null && !Number.isNaN(value);
  const clampedValue = hasValue ? clamp(value, min, max) : min;
  const ratio = hasValue ? (clampedValue - min) / range : 0;
  const progressColor =
    normalizedThresholds.find((item) => clampedValue <= item.value)?.color ?? normalizedThresholds[normalizedThresholds.length - 1]?.color ?? "#7be37c";
  let previousValue = min;
  const segments = normalizedThresholds
    .map((item) => {
      const segmentEnd = clamp(item.value, min, max);
      if (segmentEnd <= previousValue) return null;
      const startAngle = GAUGE_START_ANGLE + ((previousValue - min) / range) * GAUGE_SWEEP_ANGLE;
      const endAngle = GAUGE_START_ANGLE + ((segmentEnd - min) / range) * GAUGE_SWEEP_ANGLE;
      previousValue = segmentEnd;
      return {
        key: `${title}-${item.value}-${item.color}`,
        color: item.color,
        path: describeArc(60, 60, 40, startAngle, endAngle),
      };
    })
    .filter((item): item is { key: string; color: string; path: string } => Boolean(item));
  const progressPath =
    ratio > 0 ? describeArc(60, 60, 40, GAUGE_START_ANGLE, GAUGE_START_ANGLE + ratio * GAUGE_SWEEP_ANGLE) : "";
  const formattedValue = hasValue ? (formatter ? formatter(clampedValue) : `${clampedValue.toFixed(Math.abs(clampedValue) >= 100 ? 0 : 1)}${unit}`) : "—";

  return (
    <div className="native-gauge-card" aria-label={title}>
      <div className="native-gauge-head">
        <strong>{title}</strong>
        {detail ? <span>{detail}</span> : null}
      </div>
      <div className="native-gauge-visual">
        <svg className="native-gauge-svg" viewBox="0 0 120 92" role="img" aria-label={title}>
          <path className="native-gauge-track" d={describeArc(60, 60, 40, GAUGE_START_ANGLE, GAUGE_START_ANGLE + GAUGE_SWEEP_ANGLE)} />
          {segments.map((segment) => (
            <path key={segment.key} d={segment.path} stroke={segment.color} className="native-gauge-segment" />
          ))}
          {progressPath ? <path d={progressPath} stroke={progressColor} className="native-gauge-progress" /> : null}
        </svg>
        <div className="native-gauge-center">
          <strong>{formattedValue}</strong>
          <span>{hasValue ? `${min}${unit} to ${max}${unit}` : "no data"}</span>
        </div>
      </div>
    </div>
  );
}

export function NativeBarChart({
  title,
  rows,
  color,
  valueFormatter,
}: {
  title: string;
  rows: Array<{ label: string; value: number; tone?: string }>;
  color?: string;
  valueFormatter?: (value: number) => string;
}) {
  const max = Math.max(...rows.map((row) => row.value), 1);
  return (
    <div className="bar-list" aria-label={title}>
      {rows.map((row) => (
        <div key={row.label} className="bar-row">
          <span>{row.label}</span>
          <div className="bar-track">
            <div
              className="bar-fill"
              style={{
                width: `${Math.max(6, (row.value / max) * 100)}%`,
                background: row.tone || color || "linear-gradient(90deg, #4d9bff 0%, #7be37c 100%)",
              }}
            />
          </div>
          <strong>{valueFormatter ? valueFormatter(row.value) : row.value.toLocaleString()}</strong>
        </div>
      ))}
    </div>
  );
}
