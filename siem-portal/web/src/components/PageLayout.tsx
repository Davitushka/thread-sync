import { Link, useLocation } from "react-router-dom";
import { useEffect, useMemo, useState } from "react";
import { resolveHeaderMeta } from "../suite-meta";

export function useActorState() {
  const [actor, setActor] = useState(() => localStorage.getItem("soc_actor") || "analyst");

  useEffect(() => {
    localStorage.setItem("soc_actor", actor);
    window.dispatchEvent(new CustomEvent("suite:actor-changed", { detail: actor }));
  }, [actor]);

  useEffect(() => {
    const syncActor = (nextActor: string | null) => {
      if (!nextActor) return;
      setActor((current) => (current === nextActor ? current : nextActor));
    };

    const onStorage = (event: StorageEvent) => {
      if (event.key === "soc_actor") {
        syncActor(event.newValue);
      }
    };

    const onActorChanged = (event: Event) => {
      syncActor((event as CustomEvent<string>).detail ?? null);
    };

    window.addEventListener("storage", onStorage);
    window.addEventListener("suite:actor-changed", onActorChanged as EventListener);

    return () => {
      window.removeEventListener("storage", onStorage);
      window.removeEventListener("suite:actor-changed", onActorChanged as EventListener);
    };
  }, []);

  return { actor, setActor };
}

export function SuiteTopbar() {
  const location = useLocation();
  const meta = useMemo(() => resolveHeaderMeta(location.pathname), [location.pathname]);

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
        <div className="suite-topbar-actions">
          <button
            type="button"
            className="suite-command-btn"
            onClick={() => window.dispatchEvent(new CustomEvent("suite:open-command-palette"))}
          >
            Search or run
            <span>Ctrl+K</span>
          </button>
          {meta.mode ? <span className="suite-mode-pill">{meta.mode}</span> : null}
        </div>
      </div>
    </header>
  );
}
