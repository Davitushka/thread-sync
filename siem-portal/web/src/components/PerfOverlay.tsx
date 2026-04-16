import { useEffect, useState } from "react";

export default function PerfOverlay() {
  const [fps, setFps] = useState(0);
  const [long16, setLong16] = useState(0);
  const [long33, setLong33] = useState(0);

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
        setFps(frames);
        setLong16(l16);
        setLong33(l33);
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
      <span>FPS: {fps}</span>
      <span>&gt;16ms: {long16}/s</span>
      <span>&gt;33ms: {long33}/s</span>
    </div>
  );
}
