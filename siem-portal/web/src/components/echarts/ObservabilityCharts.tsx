import { memo, useEffect, useMemo, useRef, useState, type ReactNode } from "react";
import { useChartMotion } from "../ChartMotionContext";
import ReactEChartsCore from "echarts-for-react/lib/core";
import * as echarts from "echarts/core";
import type { EChartsCoreOption } from "echarts/core";
import { BarChart, GaugeChart, LineChart } from "echarts/charts";
import { DataZoomComponent, GridComponent, LegendComponent, TooltipComponent } from "echarts/components";
import { CanvasRenderer } from "echarts/renderers";

echarts.use([LineChart, BarChart, GaugeChart, DataZoomComponent, GridComponent, LegendComponent, TooltipComponent, CanvasRenderer]);

const PANEL_TEXT = "#f4f8ff";
const PANEL_MUTED = "#90a3bf";
const PANEL_GRID = "rgba(148, 163, 184, 0.14)";
const PANEL_AXIS = "rgba(148, 163, 184, 0.2)";
const TOOLTIP_BG = "rgba(10, 17, 27, 0.96)";
const FALLBACK_COLORS = ["#7be37c", "#4d9bff", "#f0c15d", "#8f6dff", "#f85149"];

type Threshold = { value: number; color: string };

type TimeSeries = {
  name: string;
  data: number[];
  color: string;
  areaOpacity?: number;
};

type BarRow = {
  label: string;
  value: number;
  color?: string;
};

type LinePointClick = {
  category: string;
  value: number;
  seriesName: string;
  dataIndex: number;
};

type BarRowClick = {
  label: string;
  value: number;
  dataIndex: number;
};

function joinClasses(...values: Array<string | undefined | false>) {
  return values.filter(Boolean).join(" ");
}

function defaultNumberFormatter(value: number) {
  return new Intl.NumberFormat("en", { maximumFractionDigits: 1 }).format(value);
}

function clamp(value: number, min: number, max: number) {
  return Math.min(Math.max(value, min), max);
}

function activeGaugeColor(value: number, min: number, max: number, thresholds?: Threshold[]) {
  if (!thresholds?.length) {
    const ratio = (value - min) / Math.max(max - min, 1);
    if (ratio <= 0.7) return "#7be37c";
    if (ratio <= 0.9) return "#f0c15d";
    return "#f85149";
  }
  const sorted = [...thresholds].sort((left, right) => left.value - right.value);
  return sorted.find((item) => value <= item.value)?.color ?? sorted[sorted.length - 1]?.color ?? "#7be37c";
}

function gaugeStops(min: number, max: number, thresholds?: Threshold[]) {
  if (!thresholds?.length) {
    return [
      [0.7, "#7be37c"],
      [0.9, "#f0c15d"],
      [1, "#f85149"],
    ] as Array<[number, string]>;
  }
  return [...thresholds]
    .sort((left, right) => left.value - right.value)
    .map((item) => [clamp((item.value - min) / Math.max(max - min, 1), 0, 1), item.color] as [number, string]);
}

function inOperatorWebView(): boolean {
  if (typeof window === "undefined") return false;
  const w = window as Window & { chrome?: { webview?: unknown } };
  return Boolean(w.chrome?.webview);
}

function useChartPerfMode() {
  const lowMotion = useMemo(() => {
    if (typeof window === "undefined") return false;
    const reduced = window.matchMedia?.("(prefers-reduced-motion: reduce)").matches ?? false;
    return reduced || inOperatorWebView();
  }, []);
  return {
    lowMotion,
    throttleMs: lowMotion ? 900 : 260,
  };
}

function useVisibilityGate() {
  const rootRef = useRef<HTMLDivElement | null>(null);
  const [visible, setVisible] = useState(true);

  useEffect(() => {
    const node = rootRef.current;
    if (!node || typeof IntersectionObserver === "undefined") return;
    const obs = new IntersectionObserver(
      (entries) => {
        const entry = entries[0];
        setVisible(Boolean(entry?.isIntersecting));
      },
      {
        root: null,
        rootMargin: "120px 0px 120px 0px",
        threshold: 0.01,
      }
    );
    obs.observe(node);
    return () => obs.disconnect();
  }, []);

  return { rootRef, visible };
}

function useThrottledChartOption(option: EChartsCoreOption, throttleMs: number, enabled: boolean) {
  const [gated, setGated] = useState(option);
  const pendingRef = useRef(option);
  const timerRef = useRef<number | null>(null);
  const lastAtRef = useRef(0);

  useEffect(() => {
    pendingRef.current = option;
    if (!enabled) {
      setGated(option);
      if (timerRef.current != null) {
        window.clearTimeout(timerRef.current);
        timerRef.current = null;
      }
      return;
    }

    const now = performance.now();
    const elapsed = now - lastAtRef.current;
    if (elapsed >= throttleMs) {
      lastAtRef.current = now;
      setGated(option);
      return;
    }
    if (timerRef.current != null) return;

    timerRef.current = window.setTimeout(() => {
      timerRef.current = null;
      lastAtRef.current = performance.now();
      setGated(pendingRef.current);
    }, Math.max(16, throttleMs - elapsed));
  }, [enabled, option, throttleMs]);

  useEffect(
    () => () => {
      if (timerRef.current != null) {
        window.clearTimeout(timerRef.current);
      }
    },
    []
  );

  return gated;
}

function chartBase(title: string, animationsEnabled: boolean): EChartsCoreOption {
  return {
    animation: animationsEnabled,
    animationDuration: animationsEnabled ? 350 : 0,
    animationDurationUpdate: animationsEnabled ? 250 : 0,
    textStyle: {
      color: PANEL_TEXT,
      fontFamily: "Inter, system-ui, sans-serif",
    },
    tooltip: {
      backgroundColor: TOOLTIP_BG,
      borderColor: PANEL_AXIS,
      borderWidth: 1,
      textStyle: {
        color: PANEL_TEXT,
        fontSize: 12,
      },
      extraCssText: "box-shadow: 0 10px 40px rgba(0,0,0,0.35); border-radius: 10px;",
      confine: true,
    },
    aria: {
      enabled: true,
      decal: { show: false },
    },
    title: {
      show: false,
      text: title,
    },
  };
}

export const ObservabilityPanel = memo(function ObservabilityPanel({
  title,
  subtitle,
  className,
  children,
  footer,
  kicker = "Observability panel",
}: {
  title: string;
  subtitle?: string;
  className?: string;
  children: ReactNode;
  footer?: ReactNode;
  kicker?: string;
}) {
  return (
    <section className={joinClasses("card observability-panel", className)}>
      <div className="observability-panel-header">
        <div className="observability-panel-copy">
          <span className="observability-panel-kicker">{kicker}</span>
          <h2>{title}</h2>
          {subtitle ? <p>{subtitle}</p> : null}
        </div>
      </div>
      <div className="observability-panel-body">{children}</div>
      {footer ? <div className="observability-panel-footer">{footer}</div> : null}
    </section>
  );
});

export function ObservabilityGaugePanel({
  title,
  subtitle,
  value,
  min = 0,
  max = 100,
  formatter,
  thresholds,
  className,
  footer,
  kicker,
  height = 270,
}: {
  title: string;
  subtitle?: string;
  value: number | null | undefined;
  min?: number;
  max?: number;
  formatter?: (value: number) => string;
  thresholds?: Threshold[];
  className?: string;
  footer?: ReactNode;
  kicker?: string;
  height?: number;
}) {
  const { chartAnimationsEnabled } = useChartMotion();
  const { lowMotion, throttleMs } = useChartPerfMode();
  const { rootRef, visible } = useVisibilityGate();
  const safeValue = value == null || Number.isNaN(value) ? 0 : clamp(value, min, max);
  const effectiveAnimations = chartAnimationsEnabled && !lowMotion;
  const option: EChartsCoreOption = {
    ...chartBase(title, effectiveAnimations),
    series: [
      {
        type: "gauge",
        startAngle: 210,
        endAngle: -30,
        min,
        max,
        radius: "92%",
        pointer: { show: false },
        progress: {
          show: true,
          roundCap: true,
          width: 16,
          itemStyle: {
            color: activeGaugeColor(safeValue, min, max, thresholds),
          },
        },
        axisLine: {
          roundCap: true,
          lineStyle: {
            width: 16,
            color: gaugeStops(min, max, thresholds),
          },
        },
        splitLine: { show: false },
        axisTick: { show: false },
        axisLabel: {
          color: PANEL_MUTED,
          distance: -42,
          fontSize: 11,
          formatter: (axisValue: number) => {
            if (axisValue === min) return `${min}`;
            if (axisValue === max) return `${max}`;
            return "";
          },
        },
        anchor: { show: false },
        detail: {
          valueAnimation: effectiveAnimations,
          fontSize: 30,
          fontWeight: 600,
          offsetCenter: [0, "18%"],
          color: PANEL_TEXT,
          formatter: (nextValue: number) => (formatter ? formatter(nextValue) : defaultNumberFormatter(nextValue)),
        },
        title: {
          show: false,
        },
        data: [{ value: safeValue }],
      },
    ],
  };
  const gatedOption = useThrottledChartOption(option, throttleMs, visible);

  return (
    <ObservabilityPanel title={title} subtitle={subtitle} className={className} footer={footer} kicker={kicker}>
      <div ref={rootRef} style={{ width: "100%", height }}>
        <ReactEChartsCore
          echarts={echarts}
          option={gatedOption}
          notMerge
          lazyUpdate
          opts={{ renderer: "canvas" }}
          style={{ width: "100%", height }}
        />
      </div>
    </ObservabilityPanel>
  );
}

export function ObservabilityLinePanel({
  title,
  subtitle,
  categories,
  series,
  valueFormatter,
  axisFormatter,
  className,
  footer,
  kicker,
  height = 290,
  showDataZoom = false,
  onPointClick,
}: {
  title: string;
  subtitle?: string;
  categories: string[];
  series: TimeSeries[];
  valueFormatter?: (value: number) => string;
  axisFormatter?: (value: number) => string;
  className?: string;
  footer?: ReactNode;
  kicker?: string;
  height?: number;
  showDataZoom?: boolean;
  onPointClick?: (point: LinePointClick) => void;
}) {
  const { chartAnimationsEnabled } = useChartMotion();
  const { lowMotion, throttleMs } = useChartPerfMode();
  const { rootRef, visible } = useVisibilityGate();
  const effectiveAnimations = chartAnimationsEnabled && !lowMotion;
  const chartBaseOption = chartBase(title, effectiveAnimations);
  const chartEvents = onPointClick
    ? {
        click: (params: unknown) => {
          const entry = params as {
            componentType?: string;
            name?: string;
            value?: number;
            seriesName?: string;
            dataIndex?: number;
          };
          if (entry.componentType !== "series") {
            return;
          }
          const dataIndex = Number(entry.dataIndex ?? 0);
          onPointClick({
            category: categories[dataIndex] ?? String(entry.name ?? ""),
            value: Number(entry.value ?? 0),
            seriesName: entry.seriesName ?? "value",
            dataIndex,
          });
        },
      }
    : undefined;
  const option: EChartsCoreOption = {
    ...chartBaseOption,
    grid: {
      top: series.length > 1 ? 44 : 18,
      left: 46,
      right: 18,
      bottom: showDataZoom ? 52 : 28,
    },
    legend:
      series.length > 1
        ? {
            top: 6,
            right: 6,
            icon: "roundRect",
            itemWidth: 10,
            itemHeight: 10,
            textStyle: {
              color: PANEL_MUTED,
              fontSize: 11,
            },
          }
        : undefined,
    tooltip: {
      ...(chartBaseOption.tooltip as object),
      trigger: "axis",
      axisPointer: {
        type: "line",
        lineStyle: {
          color: "rgba(148, 163, 184, 0.28)",
        },
      },
      formatter: (params: unknown) => {
        const entries = Array.isArray(params) ? params : [params];
        const safeEntries = entries as Array<{
          axisValueLabel?: string;
          marker?: string;
          seriesName?: string;
          data?: number;
        }>;
        const titleRow = safeEntries[0]?.axisValueLabel ?? "";
        const valueRows = safeEntries.map((entry) => {
          const currentValue = Number(entry.data ?? 0);
          return `${entry.marker ?? ""}${entry.seriesName ?? "value"}: ${
            valueFormatter ? valueFormatter(currentValue) : defaultNumberFormatter(currentValue)
          }`;
        });
        return [titleRow, ...valueRows].join("<br/>");
      },
    },
    xAxis: {
      type: "category",
      boundaryGap: false,
      data: categories,
      axisLine: {
        lineStyle: { color: PANEL_AXIS },
      },
      axisTick: { show: false },
      axisLabel: {
        color: PANEL_MUTED,
        fontSize: 11,
        hideOverlap: true,
      },
      splitLine: { show: false },
    },
    yAxis: {
      type: "value",
      splitNumber: 4,
      axisLine: { show: false },
      axisTick: { show: false },
      axisLabel: {
        color: PANEL_MUTED,
        fontSize: 11,
        formatter: (nextValue: number) => (axisFormatter ? axisFormatter(nextValue) : defaultNumberFormatter(nextValue)),
      },
      splitLine: {
        lineStyle: {
          color: PANEL_GRID,
        },
      },
    },
    dataZoom: showDataZoom
      ? [
          {
            type: "inside",
            xAxisIndex: 0,
            filterMode: "none",
          },
          {
            type: "slider",
            xAxisIndex: 0,
            height: 18,
            bottom: 6,
            borderColor: PANEL_AXIS,
            backgroundColor: "rgba(148, 163, 184, 0.06)",
            fillerColor: "rgba(77, 155, 255, 0.14)",
            handleStyle: {
              color: "#4d9bff",
              borderColor: "#4d9bff",
            },
            moveHandleStyle: {
              color: "#4d9bff",
            },
            textStyle: {
              color: PANEL_MUTED,
              fontSize: 10,
            },
          },
        ]
      : undefined,
    series: series.map((item, index) => ({
      name: item.name,
      type: "line",
      smooth: effectiveAnimations || categories.length <= 36,
      symbol: categories.length <= 1 ? "circle" : "none",
      symbolSize: categories.length <= 1 ? 7 : 0,
      showSymbol: categories.length <= 1,
      cursor: onPointClick ? "pointer" : "default",
      lineStyle: {
        width: 2.5,
        color: item.color || FALLBACK_COLORS[index % FALLBACK_COLORS.length],
      },
      itemStyle: {
        color: item.color || FALLBACK_COLORS[index % FALLBACK_COLORS.length],
      },
      areaStyle: item.areaOpacity
        ? {
            opacity: item.areaOpacity,
            color: item.color || FALLBACK_COLORS[index % FALLBACK_COLORS.length],
          }
        : undefined,
      emphasis: { focus: "series" },
      data: item.data,
    })),
  };
  const gatedOption = useThrottledChartOption(option, throttleMs, visible);

  return (
    <ObservabilityPanel title={title} subtitle={subtitle} className={className} footer={footer} kicker={kicker}>
      <div ref={rootRef} style={{ width: "100%", height }}>
        <ReactEChartsCore
          echarts={echarts}
          option={gatedOption}
          onEvents={chartEvents}
          notMerge
          lazyUpdate
          opts={{ renderer: "canvas" }}
          style={{ width: "100%", height }}
        />
      </div>
    </ObservabilityPanel>
  );
}

export function ObservabilityBarPanel({
  title,
  subtitle,
  rows,
  valueFormatter,
  axisFormatter,
  className,
  footer,
  kicker,
  height = 300,
  onRowClick,
}: {
  title: string;
  subtitle?: string;
  rows: BarRow[];
  valueFormatter?: (value: number) => string;
  axisFormatter?: (value: number) => string;
  className?: string;
  footer?: ReactNode;
  kicker?: string;
  height?: number;
  onRowClick?: (row: BarRowClick) => void;
}) {
  const { chartAnimationsEnabled } = useChartMotion();
  const { lowMotion, throttleMs } = useChartPerfMode();
  const { rootRef, visible } = useVisibilityGate();
  const effectiveAnimations = chartAnimationsEnabled && !lowMotion;
  const chartBaseOption = chartBase(title, effectiveAnimations);
  const chartEvents = onRowClick
    ? {
        click: (params: unknown) => {
          const entry = params as {
            componentType?: string;
            name?: string;
            value?: number;
            dataIndex?: number;
          };
          if (entry.componentType !== "series") {
            return;
          }
          const dataIndex = Number(entry.dataIndex ?? 0);
          onRowClick({
            label: rows[dataIndex]?.label ?? String(entry.name ?? ""),
            value: Number(entry.value ?? 0),
            dataIndex,
          });
        },
      }
    : undefined;
  const option: EChartsCoreOption = {
    ...chartBaseOption,
    grid: {
      top: 12,
      left: 16,
      right: 66,
      bottom: 6,
      containLabel: true,
    },
    tooltip: {
      ...(chartBaseOption.tooltip as object),
      trigger: "item",
      formatter: (params: unknown) => {
        const entry = params as { name?: string; value?: number };
        const currentValue = Number(entry.value ?? 0);
        return `${entry.name ?? ""}: ${valueFormatter ? valueFormatter(currentValue) : defaultNumberFormatter(currentValue)}`;
      },
    },
    xAxis: {
      type: "value",
      axisLine: { show: false },
      axisTick: { show: false },
      axisLabel: {
        color: PANEL_MUTED,
        fontSize: 11,
        formatter: (nextValue: number) => (axisFormatter ? axisFormatter(nextValue) : defaultNumberFormatter(nextValue)),
      },
      splitLine: {
        lineStyle: {
          color: PANEL_GRID,
        },
      },
    },
    yAxis: {
      type: "category",
      inverse: true,
      data: rows.map((row) => row.label),
      axisLine: { show: false },
      axisTick: { show: false },
      axisLabel: {
        color: PANEL_TEXT,
        fontSize: 11,
        width: 120,
        overflow: "truncate",
      },
    },
    series: [
      {
        type: "bar",
        cursor: onRowClick ? "pointer" : "default",
        data: rows.map((row, index) => ({
          value: row.value,
          itemStyle: {
            color: row.color || FALLBACK_COLORS[index % FALLBACK_COLORS.length],
            borderRadius: [0, 8, 8, 0],
          },
        })),
        barWidth: 14,
        label: {
          show: true,
          position: "right",
          color: PANEL_TEXT,
          formatter: (params: unknown) => {
            const entry = params as { value?: number };
            const currentValue = Number(entry.value ?? 0);
            return valueFormatter ? valueFormatter(currentValue) : defaultNumberFormatter(currentValue);
          },
        },
      },
    ],
  };
  const gatedOption = useThrottledChartOption(option, throttleMs, visible);

  return (
    <ObservabilityPanel title={title} subtitle={subtitle} className={className} footer={footer} kicker={kicker}>
      <div ref={rootRef} style={{ width: "100%", height }}>
        <ReactEChartsCore
          echarts={echarts}
          option={gatedOption}
          onEvents={chartEvents}
          notMerge
          lazyUpdate
          opts={{ renderer: "canvas" }}
          style={{ width: "100%", height }}
        />
      </div>
    </ObservabilityPanel>
  );
}
