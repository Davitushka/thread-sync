import { useEffect, useRef } from "react";

export default function PerfOverlay() {
  const spanRef = useRef<HTMLDivElement>(null);

  useEffect(() => {
    let mounted = true;
    let raf = 0;
    let lastTs = performance.now();
    let windowStart = lastTs;
    let frames = 0;
    let l16 = 0;
    let l33 = 0;

    const loop = (ts: number) => {
      if (!mounted) return;
      const dt = ts - lastTs;
      lastTs = ts;
      frames += 1;
      if (dt > 16.7) l16 += 1;
      if (dt > 33.3) l33 += 1;
      if (ts - windowStart >= 1000) {
        // Update all stats in a single DOM write — no re-render needed
        if (spanRef.current) {
          spanRef.current.textContent = `FPS: ${frames}  >16ms: ${l16}/s  >33ms: ${l33}/s`;
        }
        frames = 0;
        l16 = 0;
        l33 = 0;
        windowStart = ts;
      }
      raf = window.requestAnimationFrame(loop);
    };
    raf = window.requestAnimationFrame(loop);
    return () => {
      mounted = false;
      window.cancelAnimationFrame(raf);
    };
  }, []);

  return (
    <div className="suite-perf-overlay">
      <strong>Perf</strong>
      <span ref={spanRef}>FPS: 0  &gt;16ms: 0/s  &gt;33ms: 0/s</span>
    </div>
  );
}
