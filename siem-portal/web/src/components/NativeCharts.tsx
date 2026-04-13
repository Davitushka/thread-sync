type Point = { x: string; y: number };
type MultiPoint = { x: string; [key: string]: string | number };

function normalizeSingle(points: Point[], width: number, height: number) {
  if (!points.length) return "";
  const max = Math.max(...points.map((point) => point.y), 1);
  return points
    .map((point, idx) => {
      const x = (idx / Math.max(points.length - 1, 1)) * width;
      const y = height - (point.y / max) * height;
      return `${x.toFixed(2)},${y.toFixed(2)}`;
    })
    .join(" ");
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

export function NativeLineChart({
  title,
  points,
  color,
}: {
  title: string;
  points: Point[];
  color: string;
}) {
  const polyline = normalizeSingle(points, 100, 46);
  return (
    <div className="native-chart-shell">
      <svg className="native-chart-svg" viewBox="0 0 100 46" role="img" aria-label={title}>
        <polyline className="sparkline-gridline" points="0,45 100,45" />
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
      <svg className="native-chart-svg" viewBox="0 0 100 46" role="img" aria-label={title}>
        <polyline className="sparkline-gridline" points="0,45 100,45" />
        {series.map((item) => {
          const polyline = normalizeMulti(points, item.key, 100, 46, max);
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
