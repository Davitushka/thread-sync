import { Children, Fragment, useCallback, useEffect, useMemo, useRef, useState, type PointerEvent, type ReactNode } from "react";

type Props = {
  storageKey: string;
  defaultSizes: number[];
  minSizes?: number[];
  stackBelow?: number;
  className?: string;
  children: ReactNode;
};

const STORAGE_PREFIX = "suite_adaptive_panes";
const DEFAULT_STACK_BELOW = 1240;

function normalize(values: number[], count: number) {
  if (!count) return [];
  const filtered = values.filter((value) => Number.isFinite(value) && value > 0);
  const base = filtered.length === count ? filtered : Array.from({ length: count }, () => 1 / count);
  const total = base.reduce((sum, value) => sum + value, 0);
  return base.map((value) => value / total);
}

function readPersistedSizes(storageKey: string, fallback: number[]) {
  try {
    const raw = localStorage.getItem(`${STORAGE_PREFIX}:${storageKey}`);
    if (!raw) return fallback;
    const parsed = JSON.parse(raw) as { sizes?: number[] };
    if (!parsed.sizes?.length) return fallback;
    return normalize(parsed.sizes, fallback.length);
  } catch {
    return fallback;
  }
}

export default function AdaptivePaneLayout({
  storageKey,
  defaultSizes,
  minSizes,
  stackBelow = DEFAULT_STACK_BELOW,
  className,
  children,
}: Props) {
  const panes = useMemo(() => Children.toArray(children).filter(Boolean), [children]);
  const paneCount = panes.length;
  const defaultKey = defaultSizes.join("|");
  const minKey = (minSizes ?? []).join("|");
  const normalizedDefaults = useMemo(() => normalize(defaultSizes, paneCount), [defaultKey, paneCount]);
  const normalizedMins = useMemo(
    () => normalize(minSizes ?? Array.from({ length: paneCount }, () => 0.16), paneCount),
    [minKey, paneCount]
  );
  const [sizes, setSizes] = useState<number[]>(() => readPersistedSizes(storageKey, normalizedDefaults));
  const [isStacked, setIsStacked] = useState<boolean>(() =>
    typeof window === "undefined" ? false : window.innerWidth < stackBelow
  );
  const containerRef = useRef<HTMLDivElement | null>(null);

  useEffect(() => {
    setSizes((current) => (current.length === paneCount ? current : normalizedDefaults));
  }, [normalizedDefaults, paneCount]);

  useEffect(() => {
    const onResize = () => setIsStacked(window.innerWidth < stackBelow);
    window.addEventListener("resize", onResize);
    return () => window.removeEventListener("resize", onResize);
  }, [stackBelow]);

  useEffect(() => {
    localStorage.setItem(`${STORAGE_PREFIX}:${storageKey}`, JSON.stringify({ sizes }));
  }, [storageKey, sizes]);

  const resetLayout = useCallback(() => {
    setSizes(normalizedDefaults);
  }, [normalizedDefaults]);

  const startResize = useCallback(
    (handleIndex: number, event: PointerEvent<HTMLButtonElement>) => {
      if (isStacked || !containerRef.current) return;
      event.preventDefault();
      const startX = event.clientX;
      const startSizes = [...sizes];
      const containerWidth = containerRef.current.getBoundingClientRect().width;
      const minLeft = normalizedMins[handleIndex] ?? 0.12;
      const minRight = normalizedMins[handleIndex + 1] ?? 0.12;

      document.body.style.cursor = "col-resize";
      document.body.style.userSelect = "none";

      const onMove = (moveEvent: PointerEvent) => {
        const deltaRatio = (moveEvent.clientX - startX) / Math.max(containerWidth, 1);
        const pairTotal = startSizes[handleIndex] + startSizes[handleIndex + 1];
        const nextLeft = Math.max(minLeft, Math.min(pairTotal - minRight, startSizes[handleIndex] + deltaRatio));
        const next = [...startSizes];
        next[handleIndex] = nextLeft;
        next[handleIndex + 1] = pairTotal - nextLeft;
        setSizes(next);
      };

      const onUp = () => {
        document.body.style.cursor = "";
        document.body.style.userSelect = "";
        window.removeEventListener("pointermove", onMove);
        window.removeEventListener("pointerup", onUp);
      };

      window.addEventListener("pointermove", onMove);
      window.addEventListener("pointerup", onUp);
    },
    [isStacked, normalizedMins, sizes]
  );

  const gridTemplateColumns = useMemo(() => {
    if (isStacked || !sizes.length) return undefined;
    return sizes.map((size, index) => `minmax(0, ${size}fr)${index < sizes.length - 1 ? " 12px" : ""}`).join(" ");
  }, [isStacked, sizes]);

  return (
    <div
      ref={containerRef}
      className={["adaptive-pane-layout", isStacked ? "stacked" : "", className].filter(Boolean).join(" ")}
      style={gridTemplateColumns ? { gridTemplateColumns } : undefined}
    >
      {panes.map((pane, index) => (
        <Fragment key={`${storageKey}:${index}`}>
          <div className="adaptive-pane">{pane}</div>
          {index < panes.length - 1 && !isStacked ? (
            <button
              type="button"
              className="adaptive-pane-handle"
              aria-label="Resize panes"
              onPointerDown={(event) => startResize(index, event)}
              onDoubleClick={resetLayout}
            >
              <span className="adaptive-pane-grip" />
            </button>
          ) : null}
        </Fragment>
      ))}
    </div>
  );
}
