import {
  createContext,
  useCallback,
  useContext,
  useEffect,
  useMemo,
  useRef,
  useState,
  type ReactNode,
} from "react";

type WsIncoming =
  | { type: "welcome"; protocol?: number; poll_ms?: number; server?: string }
  | { type: "snapshot"; topic: string; at_ms?: number; data: unknown }
  | { type: "error"; topic: string; message: string }
  | { type: "pong"; nonce?: number };

export type SuiteRealtimeConnection = "idle" | "connecting" | "open" | "closed";

export type SuiteRealtimeContextValue = {
  connection: SuiteRealtimeConnection;
  /** When WebSocket is healthy, HTTP polling intervals should be skipped. */
  shouldPoll: boolean;
  lastError: string | null;
  subscribe: (topics: readonly string[], listener: (topic: string, data: unknown) => void) => () => void;
};

const SuiteRealtimeContext = createContext<SuiteRealtimeContextValue | null>(null);

function routerBasePath(): string {
  const raw = (import.meta.env.BASE_URL as string | undefined)?.trim() || "/";
  if (raw === "/") return "";
  return raw.replace(/\/+$/, "") || "";
}

function defaultWsPath(): string {
  return (import.meta.env.VITE_SUITE_WS_PATH as string | undefined)?.trim() || "/api/v1/realtime/ws";
}

function buildWebSocketUrl(): string {
  const proto = window.location.protocol === "https:" ? "wss" : "ws";
  const base = routerBasePath();
  const path = defaultWsPath().startsWith("/") ? defaultWsPath() : `/${defaultWsPath()}`;
  return `${proto}://${window.location.host}${base}${path}`;
}

const REALTIME_DISABLED = (import.meta.env.VITE_SUITE_REALTIME as string | undefined)?.trim() === "0";

export function useSuiteRealtime(): SuiteRealtimeContextValue {
  const v = useContext(SuiteRealtimeContext);
  if (!v) {
    throw new Error("useSuiteRealtime must be used within SuiteRealtimeProvider");
  }
  return v;
}

/** Safe when provider is absent (e.g. tests). */
export function useSuiteRealtimeOptional(): SuiteRealtimeContextValue | null {
  return useContext(SuiteRealtimeContext);
}

/** When the suite WebSocket is live, return 0 so `useVisibleInterval` does not duplicate upstream polls. */
export function useEffectivePollingInterval(intervalSec: number): number {
  const rt = useSuiteRealtimeOptional();
  if (intervalSec === 0) return 0;
  if (!rt || rt.shouldPoll) return intervalSec;
  return 0;
}

export function useSuiteRealtimeTopics(
  topics: readonly string[],
  onSnapshot: (topic: string, data: unknown) => void
): void {
  const rt = useSuiteRealtimeOptional();
  const onRef = useRef(onSnapshot);
  onRef.current = onSnapshot;
  const key = topics.join("\u0001");

  useEffect(() => {
    if (!rt || topics.length === 0) return;
    return rt.subscribe(topics, (t, d) => onRef.current(t, d));
  }, [rt, key, topics]);
}

export function SuiteRealtimeProvider({ children }: { children: ReactNode }) {
  const [connection, setConnection] = useState<SuiteRealtimeConnection>(REALTIME_DISABLED ? "closed" : "idle");
  const [lastError, setLastError] = useState<string | null>(null);

  const listeners = useRef(new Map<string, Set<(data: unknown) => void>>());
  const wireRef = useRef(new Map<string, number>());
  const wsRef = useRef<WebSocket | null>(null);
  const reconnectTimer = useRef<number | null>(null);
  const backoffRef = useRef(1_000);
  const pingTimer = useRef<number | null>(null);

  const notify = useCallback((topic: string, data: unknown) => {
    const set = listeners.current.get(topic);
    if (!set) return;
    for (const fn of set) {
      try {
        fn(data);
      } catch (e) {
        console.error("suite realtime listener", e);
      }
    }
  }, []);

  const sendWire = useCallback((raw: object) => {
    const ws = wsRef.current;
    if (!ws || ws.readyState !== WebSocket.OPEN) return;
    ws.send(JSON.stringify(raw));
  }, []);

  const flushSubscribe = useCallback(
    (topics: readonly string[]) => {
      const fresh: string[] = [];
      for (const t of topics) {
        if (!t) continue;
        const n = wireRef.current.get(t) ?? 0;
        if (n === 0) fresh.push(t);
        wireRef.current.set(t, n + 1);
      }
      if (fresh.length) sendWire({ type: "subscribe", topics: fresh });
    },
    [sendWire]
  );

  const flushUnsubscribe = useCallback(
    (topics: readonly string[]) => {
      const gone: string[] = [];
      for (const t of topics) {
        const n = wireRef.current.get(t);
        if (n == null) continue;
        if (n <= 1) {
          wireRef.current.delete(t);
          gone.push(t);
        } else {
          wireRef.current.set(t, n - 1);
        }
      }
      if (gone.length) sendWire({ type: "unsubscribe", topics: gone });
    },
    [sendWire]
  );

  const subscribe = useCallback(
    (topics: readonly string[], listener: (topic: string, data: unknown) => void) => {
      const active = topics.filter(Boolean);
      const perTopicFns = new Map<string, (data: unknown) => void>();
      for (const t of active) {
        const fn = (data: unknown) => listener(t, data);
        perTopicFns.set(t, fn);
        let set = listeners.current.get(t);
        if (!set) {
          set = new Set();
          listeners.current.set(t, set);
        }
        set.add(fn);
      }
      flushSubscribe(active);

      return () => {
        for (const [t, fn] of perTopicFns) {
          const set = listeners.current.get(t);
          if (set) {
            set.delete(fn);
            if (set.size === 0) listeners.current.delete(t);
          }
        }
        flushUnsubscribe(active);
      };
    },
    [flushSubscribe, flushUnsubscribe]
  );

  const value = useMemo<SuiteRealtimeContextValue>(
    () => ({
      connection,
      shouldPoll: REALTIME_DISABLED || connection !== "open",
      lastError,
      subscribe,
    }),
    [connection, lastError, subscribe]
  );

  useEffect(() => {
    if (REALTIME_DISABLED) return;

    let stopped = false;

    const clearTimers = () => {
      if (reconnectTimer.current != null) {
        window.clearTimeout(reconnectTimer.current);
        reconnectTimer.current = null;
      }
      if (pingTimer.current != null) {
        window.clearInterval(pingTimer.current);
        pingTimer.current = null;
      }
    };

    const scheduleReconnect = () => {
      if (stopped) return;
      const delay = Math.min(backoffRef.current, 30_000);
      backoffRef.current = Math.min(backoffRef.current * 2, 30_000);
      reconnectTimer.current = window.setTimeout(() => connect(), delay);
    };

    const connect = () => {
      if (stopped) return;
      clearTimers();
      setConnection("connecting");
      setLastError(null);
      const url = buildWebSocketUrl();
      let ws: WebSocket;
      try {
        ws = new WebSocket(url);
      } catch (e) {
        setLastError(String(e));
        setConnection("closed");
        scheduleReconnect();
        return;
      }
      wsRef.current = ws;

      ws.onopen = () => {
        if (stopped) return;
        backoffRef.current = 1_000;
        setConnection("open");
        const topics = [...wireRef.current.keys()];
        if (topics.length) {
          ws.send(JSON.stringify({ type: "subscribe", topics }));
        }
        pingTimer.current = window.setInterval(() => {
          if (ws.readyState === WebSocket.OPEN) {
            ws.send(JSON.stringify({ type: "ping", nonce: Date.now() }));
          }
        }, 45_000);
      };

      ws.onmessage = (ev) => {
        if (typeof ev.data !== "string") return;
        let msg: WsIncoming;
        try {
          msg = JSON.parse(ev.data) as WsIncoming;
        } catch {
          return;
        }
        if (msg.type === "snapshot") {
          notify(msg.topic, msg.data);
        } else if (msg.type === "error") {
          setLastError(`${msg.topic}: ${msg.message}`);
        }
      };

      ws.onerror = () => {
        setLastError("WebSocket error");
      };

      ws.onclose = () => {
        wsRef.current = null;
        clearTimers();
        if (stopped) return;
        setConnection("closed");
        scheduleReconnect();
      };
    };

    connect();

    return () => {
      stopped = true;
      clearTimers();
      const ws = wsRef.current;
      wsRef.current = null;
      if (ws && (ws.readyState === WebSocket.OPEN || ws.readyState === WebSocket.CONNECTING)) {
        ws.close();
      }
    };
  }, [notify]);

  return <SuiteRealtimeContext.Provider value={value}>{children}</SuiteRealtimeContext.Provider>;
}
