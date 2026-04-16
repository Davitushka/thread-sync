import React, { type ReactNode } from "react";
import { DASHBOARD_WINDOWS } from "../dashboard-utils";
import { suiteRefreshSelectOptions, type SuiteRefreshChoice } from "../suite-polling";

type Props = {
  title: string;
  subtitle: string;
  hours?: number;
  autoRefreshSec?: SuiteRefreshChoice;
  loading?: boolean;
  onHoursChange?: (hours: number) => void;
  onAutoRefreshChange?: (sec: SuiteRefreshChoice) => void;
  onRefresh?: () => void;
  actions?: ReactNode;
  children?: ReactNode;
  rangeOptions?: ReadonlyArray<{ value: number; label: string }>;
  rangeLabel?: string;
  refreshLabel?: string;
  refreshButtonLabel?: string;
  className?: string;
};

const REFRESH_OPTIONS = suiteRefreshSelectOptions();

export default React.memo(function DashboardToolbar({
  title,
  subtitle,
  hours,
  autoRefreshSec,
  loading,
  onHoursChange,
  onAutoRefreshChange,
  onRefresh,
  actions,
  children,
  rangeOptions = DASHBOARD_WINDOWS,
  rangeLabel = "Range",
  refreshLabel = "Refresh",
  refreshButtonLabel = "Refresh now",
  className,
}: Props) {
  const showRangeControl = typeof hours === "number" && typeof onHoursChange === "function";
  const showRefreshSelect = typeof autoRefreshSec === "number" && typeof onAutoRefreshChange === "function";
  const showRefreshButton = typeof onRefresh === "function";

  return (
    <section className={["card", "hero-card", "dashboard-toolbar-card", className].filter(Boolean).join(" ")}>
      <div className="dashboard-hero">
        <div>
          <h2>{title}</h2>
          <p className="meta">{subtitle}</p>
        </div>
        <div className="dashboard-toolbar-side">
          {actions ? <div className="dashboard-toolbar-actions">{actions}</div> : null}
          {showRangeControl || showRefreshSelect || showRefreshButton ? (
            <div className="dashboard-controls">
              {showRangeControl ? (
                <label>
                  {rangeLabel}
                  <select value={hours} onChange={(e) => onHoursChange(Number(e.target.value))}>
                    {rangeOptions.map((option) => (
                      <option key={option.value} value={option.value}>
                        {option.label}
                      </option>
                    ))}
                  </select>
                </label>
              ) : null}
              {showRefreshSelect ? (
                <label>
                  {refreshLabel}
                  <select
                    value={autoRefreshSec}
                    onChange={(e) => onAutoRefreshChange(Number(e.target.value) as SuiteRefreshChoice)}
                  >
                    {REFRESH_OPTIONS.map((option) => (
                      <option key={option.value} value={option.value}>
                        {option.label}
                      </option>
                    ))}
                  </select>
                </label>
              ) : null}
              {showRefreshButton ? (
                <button type="button" className="secondary" onClick={onRefresh} disabled={loading}>
                  {loading ? "Refreshing..." : refreshButtonLabel}
                </button>
              ) : null}
            </div>
          ) : null}
        </div>
      </div>
      {children ? <div className="dashboard-toolbar-body">{children}</div> : null}
    </section>
  );
});
