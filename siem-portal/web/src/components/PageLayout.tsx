import { Link, useLocation, matchPath } from "react-router-dom";
import { useEffect, useMemo, useState } from "react";

type HeaderMeta = {
  title: string;
  subtitle: string;
  crumbs: Array<{ label: string; to?: string }>;
  mode?: string;
};

const ROUTE_META: Array<{ path: string; end: boolean; meta: HeaderMeta }> = [
  {
    path: "/",
    end: true,
    meta: {
      title: "SOC overview",
      subtitle: "Ключевые сигналы, состояние стека и быстрые переходы для ежедневной работы.",
      crumbs: [{ label: "Overview" }],
      mode: "suite",
    },
  },
  {
    path: "/infrastructure",
    end: true,
    meta: {
      title: "Infrastructure",
      subtitle: "Host, network, containers и health-сигналы платформы.",
      crumbs: [{ label: "Infrastructure" }],
      mode: "ops",
    },
  },
  {
    path: "/dashboards",
    end: true,
    meta: {
      title: "Dashboards",
      subtitle: "Каталог deep-dive dashboard-ов и fallback на embedded Grafana.",
      crumbs: [{ label: "Dashboards" }],
      mode: "analytics",
    },
  },
  {
    path: "/alerts",
    end: true,
    meta: {
      title: "Alerts console",
      subtitle: "Плотный triage inbox для алертов, очередей и быстрых действий.",
      crumbs: [{ label: "Alerts" }],
      mode: "triage",
    },
  },
  {
    path: "/detections",
    end: true,
    meta: {
      title: "Detections console",
      subtitle: "Engine health, firing rules и noisy signals в одном экране.",
      crumbs: [{ label: "Detections" }],
      mode: "triage",
    },
  },
  {
    path: "/events",
    end: true,
    meta: {
      title: "Event search",
      subtitle: "Native ClickHouse search, pivots и event detail внутри suite.",
      crumbs: [{ label: "Events" }],
      mode: "hunt",
    },
  },
  {
    path: "/cases",
    end: true,
    meta: {
      title: "Cases",
      subtitle: "Единый case workflow поверх portal BFF и case-management.",
      crumbs: [{ label: "Cases" }],
      mode: "casework",
    },
  },
  {
    path: "/cases/:id",
    end: true,
    meta: {
      title: "Case detail",
      subtitle: "Управление статусом, timeline и связанными signal-ами.",
      crumbs: [{ label: "Cases", to: "/cases" }, { label: "Case detail" }],
      mode: "casework",
    },
  },
  {
    path: "/cases/:id/investigate",
    end: true,
    meta: {
      title: "Investigation workbench",
      subtitle: "Сводка кейса, merged feed и investigative pivots.",
      crumbs: [{ label: "Cases", to: "/cases" }, { label: "Investigation" }],
      mode: "investigation",
    },
  },
];

export function useActorState() {
  const [actor, setActor] = useState(() => localStorage.getItem("soc_actor") || "analyst");

  useEffect(() => {
    localStorage.setItem("soc_actor", actor);
  }, [actor]);

  return { actor, setActor };
}

export function SuiteTopbar() {
  const location = useLocation();
  const meta = useMemo(() => {
    return (
      ROUTE_META.find((entry) => matchPath({ path: entry.path, end: entry.end }, location.pathname))?.meta ??
      {
        title: "Unified Analyst Suite",
        subtitle: "Один вход для мониторинга, triage, расследований и кейсов.",
        crumbs: [{ label: "Suite" }],
        mode: "suite",
      }
    );
  }, [location.pathname]);

  return (
    <header className="suite-topbar">
      <div className="suite-topbar-head">
        <div>
          <div className="suite-crumbs">
            {meta.crumbs.map((crumb, idx) =>
              crumb.to ? (
                <span key={`${crumb.label}-${idx}`}>
                  <Link to={crumb.to}>{crumb.label}</Link>
                </span>
              ) : (
                <span key={`${crumb.label}-${idx}`}>{crumb.label}</span>
              )
            )}
          </div>
          <h1>{meta.title}</h1>
          <p>{meta.subtitle}</p>
        </div>
        {meta.mode ? <span className="suite-mode-pill">{meta.mode}</span> : null}
      </div>
    </header>
  );
}
