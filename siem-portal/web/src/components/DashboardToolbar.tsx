import { DASHBOARD_WINDOWS } from "../dashboard-utils";

type Props = {
  title: string;
  subtitle: string;
  hours: number;
  autoRefreshSec: number;
  loading?: boolean;
  onHoursChange: (hours: number) => void;
  onAutoRefreshChange: (sec: number) => void;
  onRefresh: () => void;
};

const REFRESH_OPTIONS = [
  { value: 0, label: "manual" },
  { value: 15, label: "15s" },
  { value: 30, label: "30s" },
  { value: 60, label: "1m" },
  { value: 300, label: "5m" },
] as const;

export default function DashboardToolbar({
  title,
  subtitle,
  hours,
  autoRefreshSec,
  loading,
  onHoursChange,
  onAutoRefreshChange,
  onRefresh,
}: Props) {
  return (
    <section className="card hero-card">
      <div className="dashboard-hero">
        <div>
          <h2>{title}</h2>
          <p className="meta">{subtitle}</p>
        </div>
        <div className="dashboard-controls">
          <label>
            Range
            <select value={hours} onChange={(e) => onHoursChange(Number(e.target.value))}>
              {DASHBOARD_WINDOWS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </label>
          <label>
            Refresh
            <select value={autoRefreshSec} onChange={(e) => onAutoRefreshChange(Number(e.target.value))}>
              {REFRESH_OPTIONS.map((option) => (
                <option key={option.value} value={option.value}>
                  {option.label}
                </option>
              ))}
            </select>
          </label>
          <button type="button" className="secondary" onClick={onRefresh} disabled={loading}>
            {loading ? "Refreshing..." : "Refresh now"}
          </button>
        </div>
      </div>
    </section>
  );
}
