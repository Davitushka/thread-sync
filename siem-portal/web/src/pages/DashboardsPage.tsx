import { useEffect, useMemo, useState } from "react";
import { uiConfig, type UiConfig } from "../api";

type DashboardEntry = {
  id: string;
  group: "SOC Core" | "Platform" | "Deep Dive";
  title: string;
  uid: string;
  description: string;
};

const DASHBOARDS: DashboardEntry[] = [
  {
    id: "overview",
    group: "SOC Core",
    title: "SIEM Overview",
    uid: "siem-overview",
    description: "Главный SOC-обзор: события, severity mix, top source IPs и recent security events.",
  },
  {
    id: "detections",
    group: "SOC Core",
    title: "Detection",
    uid: "siem-detection",
    description: "Угрозы, активные алерты, уникальные атакующие IP и throughput детекции.",
  },
  {
    id: "alerts",
    group: "SOC Core",
    title: "Alert Management",
    uid: "siem-alerts",
    description: "Очередь алертов, false positive rate, SLA и детальные таблицы по alert lifecycle.",
  },
  {
    id: "workbench",
    group: "SOC Core",
    title: "SOC Workbench",
    uid: "siem-soc-workbench",
    description: "IoC, кейсы и analyst-facing workbench для связанного deep-dive.",
  },
  {
    id: "infrastructure",
    group: "Platform",
    title: "Infrastructure",
    uid: "siem-infrastructure",
    description: "CPU, RAM, disk, network, контейнеры и общее состояние платформы.",
  },
  {
    id: "operations",
    group: "Platform",
    title: "Operations",
    uid: "siem-operations",
    description: "Vector, parser, Redpanda, ClickHouse, Grafana, Alertmanager и сквозной pipeline.",
  },
  {
    id: "validation",
    group: "Platform",
    title: "Validation",
    uid: "siem-validation",
    description: "Проверки стека и подсказки, почему отдельные панели могут быть пустыми.",
  },
  {
    id: "data-quality",
    group: "Platform",
    title: "Data Quality",
    uid: "siem-data-quality",
    description: "Качество и задержки данных: parser success/error, lag ingest и consumer lag.",
  },
  {
    id: "clickhouse-data",
    group: "Deep Dive",
    title: "ClickHouse Data Analysis",
    uid: "ch-data-analysis-sql",
    description: "SQL-анализ `siem.events` и `system.query_log` прямо в Grafana dashboard.",
  },
  {
    id: "clickhouse-query",
    group: "Deep Dive",
    title: "ClickHouse Query Analysis",
    uid: "ch-query-analysis-sql",
    description: "Slow queries, heavy statements и профиль запросов ClickHouse.",
  },
  {
    id: "correlator",
    group: "Deep Dive",
    title: "Correlator Metrics",
    uid: "siem-correlator-metrics",
    description: "Метрики коррелятора: обработка событий, firing alerts и техническое здоровье движка.",
  },
  {
    id: "prometheus",
    group: "Deep Dive",
    title: "Prometheus Stats",
    uid: "siem-prometheus-stats",
    description: "Scrape health, sample volume и внутренние метрики самого Prometheus.",
  },
];

const GROUPS = ["SOC Core", "Platform", "Deep Dive"] as const;
const TIME_RANGES = [
  { value: "now-6h", label: "Last 6h" },
  { value: "now-24h", label: "Last 24h" },
  { value: "now-7d", label: "Last 7d" },
  { value: "now-30d", label: "Last 30d" },
];

function grafanaDashboardUrl(root: string, uid: string, from: string, embedded: boolean): string {
  const base = root.replace(/\/$/, "");
  const params = new URLSearchParams({
    orgId: "1",
    theme: "dark",
    from,
    to: "now",
  });
  if (embedded) {
    params.set("kiosk", "tv");
  }
  return `${base}/d/${uid}?${params.toString()}`;
}

export default function DashboardsPage() {
  const [config, setConfig] = useState<UiConfig | null>(null);
  const [err, setErr] = useState<string | null>(null);
  const [group, setGroup] = useState<(typeof GROUPS)[number]>("SOC Core");
  const [selectedId, setSelectedId] = useState<string>("overview");
  const [timeRange, setTimeRange] = useState<string>("now-24h");

  useEffect(() => {
    uiConfig()
      .then(setConfig)
      .catch((e) => setErr(String(e)));
  }, []);

  const items = useMemo(() => DASHBOARDS.filter((item) => item.group === group), [group]);

  useEffect(() => {
    if (!items.some((item) => item.id === selectedId)) {
      setSelectedId(items[0]?.id ?? "overview");
    }
  }, [items, selectedId]);

  const current = useMemo(
    () => DASHBOARDS.find((item) => item.id === selectedId) ?? DASHBOARDS[0],
    [selectedId]
  );
  const grafanaRoot = config?.links.grafana || "";
  const embedUrl = grafanaRoot ? grafanaDashboardUrl(grafanaRoot, current.uid, timeRange, true) : "";
  const openUrl = grafanaRoot ? grafanaDashboardUrl(grafanaRoot, current.uid, timeRange, false) : "";

  return (
    <div className="page-grid dashboard-page">
      {err && <p className="error">{err}</p>}

      <section className="card hero-card">
        <h2>Grafana dashboards inside Unified Suite</h2>
        <p className="meta">
          Все основные дашборды Grafana теперь доступны как отдельный раздел внутри `siem-portal` и,
          соответственно, прямо внутри `siem-operator` WebView.
        </p>
        <div className="btn-row">
          <a className="tool-btn" href={openUrl || config?.links.grafana || "#"} target="_blank" rel="noreferrer">
            Open current in Grafana
          </a>
          <a className="tool-btn secondary" href={config?.links.grafana || "#"} target="_blank" rel="noreferrer">
            Open Grafana root
          </a>
        </div>
      </section>

      <section className="card">
        <div className="dashboard-toolbar">
          <div className="dashboard-tabs">
            {GROUPS.map((tab) => (
              <button
                key={tab}
                type="button"
                className={tab === group ? "tab-btn active" : "tab-btn secondary"}
                onClick={() => setGroup(tab)}
              >
                {tab}
              </button>
            ))}
          </div>
          <label>
            Time range
            <select value={timeRange} onChange={(e) => setTimeRange(e.target.value)}>
              {TIME_RANGES.map((range) => (
                <option key={range.value} value={range.value}>
                  {range.label}
                </option>
              ))}
            </select>
          </label>
        </div>

        <div className="dashboard-catalog">
          {items.map((item) => (
            <button
              key={item.id}
              type="button"
              className={item.id === current.id ? "dashboard-card active" : "dashboard-card"}
              onClick={() => setSelectedId(item.id)}
            >
              <strong>{item.title}</strong>
              <span>{item.description}</span>
            </button>
          ))}
        </div>
      </section>

      <section className="card dashboard-shell">
        <div className="dashboard-frame-header">
          <div>
            <h2>{current.title}</h2>
            <p className="meta">{current.description}</p>
          </div>
          <div className="btn-row tight">
            <a className="tool-btn inline secondary" href={openUrl || "#"} target="_blank" rel="noreferrer">
              Pop out
            </a>
          </div>
        </div>

        {!grafanaRoot ? (
          <p className="error">`links.grafana` не пришёл из `GET /api/v1/ui/config`, поэтому iframe собрать нельзя.</p>
        ) : (
          <>
            <p className="meta dashboard-note">
              Если iframe пустой, проверь `GF_SECURITY_ALLOW_EMBEDDING` и вход в Grafana. Для локального
              docker-compose раздел уже рассчитан на встроенный просмотр в Unified Suite.
            </p>
            <iframe
              key={`${current.uid}-${timeRange}`}
              className="dashboard-frame"
              title={`Grafana dashboard ${current.title}`}
              src={embedUrl}
            />
          </>
        )}
      </section>
    </div>
  );
}
