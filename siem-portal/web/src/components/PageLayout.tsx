import { Link, useLocation } from "react-router-dom";
import { useEffect, useMemo, useState } from "react";
import { resolveHeaderMeta } from "../suite-meta";
import ShellIcon from "./ShellIcon";
import { useWorkspaceShell } from "./WorkspaceShellContext";

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
  const { activeWorkspace, tabs, recentPaths, reopenRecentWorkspace } = useWorkspaceShell();
  const visibleTabCount = tabs.filter(
    (tab) => tab.path === location.pathname || tab.pinned || tab.workspaceKind === "document"
  ).length;
  const hiddenTabCount = Math.max(0, tabs.length - visibleTabCount);
  const meta = useMemo(
    () =>
      resolveHeaderMeta(location.pathname, {
        title: activeWorkspace.title,
        subtitle: activeWorkspace.subtitle,
        crumbs: activeWorkspace.crumbs,
        mode: activeWorkspace.mode,
      }),
    [activeWorkspace, location.pathname]
  );

  useEffect(() => {
    document.title = `${activeWorkspace.title} | SIEM-Lite Operator`;
  }, [activeWorkspace.title]);

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
          <div className="suite-title-row">
            <span className="suite-title-icon-shell">
              <ShellIcon iconKey={activeWorkspace.iconKey} className="suite-title-icon" size={20} />
            </span>
            <h1>{meta.title}</h1>
          </div>
          <p>{meta.subtitle}</p>
        </div>
        <div className="suite-topbar-actions">
          <div className="suite-topbar-status">
            <span className="suite-tab-count">
              {visibleTabCount} visible
              {hiddenTabCount ? ` · ${hiddenTabCount} background` : ""}
            </span>
            {recentPaths[0] && recentPaths[0] !== location.pathname ? (
              <button type="button" className="suite-reopen-btn" onClick={() => reopenRecentWorkspace(recentPaths[0])}>
                Reopen recent
              </button>
            ) : null}
          </div>
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
