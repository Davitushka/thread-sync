import { Suspense, lazy, useEffect, useState, type ReactNode } from "react";
import { createPortal } from "react-dom";
import { Link, NavLink, Route, Routes, useLocation } from "react-router-dom";
import { SuiteTopbar, useActorState } from "./components/PageLayout";
import CommandPalette from "./components/CommandPalette";
import ShellIcon from "./components/ShellIcon";
import { SuiteCommandProvider } from "./components/SuiteCommandContext";
import { WorkspaceShellProvider, useWorkspaceShell } from "./components/WorkspaceShellContext";
import { SUITE_NAV_GROUPS, resolveNavSelection } from "./suite-meta";

const OverviewPage = lazy(() => import("./pages/OverviewPage"));
const InfrastructurePage = lazy(() => import("./pages/InfrastructurePage"));
const OperationsPage = lazy(() => import("./pages/OperationsPage"));
const DataQualityPage = lazy(() => import("./pages/DataQualityPage"));
const ValidationPage = lazy(() => import("./pages/ValidationPage"));
const DashboardsPage = lazy(() => import("./pages/DashboardsPage"));
const AlertsPage = lazy(() => import("./pages/AlertsPage"));
const DetectionsPage = lazy(() => import("./pages/DetectionsPage"));
const EventsPage = lazy(() => import("./pages/EventsPage"));
const CasesList = lazy(() => import("./pages/CasesList"));
const CaseDetail = lazy(() => import("./pages/CaseDetail"));
const InvestigationWorkbench = lazy(() => import("./pages/InvestigationWorkbench"));

function WorkspaceLoadingFallback({
  title,
  subtitle,
}: {
  title: string;
  subtitle: string;
}) {
  return (
    <section className="card workspace-pane">
      <div className="workspace-pane-header">
        <div className="workspace-pane-copy">
          <span className="workspace-pane-kicker">Loading workspace</span>
          <h2>{title}</h2>
          <p className="workspace-pane-subtitle">{subtitle}</p>
        </div>
      </div>
    </section>
  );
}

function WorkspaceRoute({
  title,
  subtitle,
  children,
}: {
  title: string;
  subtitle: string;
  children: ReactNode;
}) {
  return (
    <Suspense fallback={<WorkspaceLoadingFallback title={title} subtitle={subtitle} />}>
      {children}
    </Suspense>
  );
}

function UnknownRouteFallback() {
  const location = useLocation();

  return (
    <section className="card workspace-pane workspace-route-miss">
      <div className="workspace-pane-header">
        <div className="workspace-pane-copy">
          <span className="workspace-pane-kicker">Route handoff</span>
          <h2>Workspace not found</h2>
          <p className="workspace-pane-subtitle">
            The suite booted correctly, but there is no registered workspace for <code>{location.pathname}</code>.
          </p>
        </div>
      </div>
      <div className="workspace-route-miss-actions">
        <Link className="tool-btn" to="/">
          Return to overview
        </Link>
        <Link className="tool-btn secondary" to="/cases">
          Open case queue
        </Link>
      </div>
    </section>
  );
}


type SidebarWorkspace = {
  path: string;
  tabLabel: string;
  description: string;
  iconKey: string;
  pinned: boolean;
  isOpen: boolean;
  workspaceKind: "workspace" | "document";
};

function WorkspaceShortcutSection({
  title,
  items,
  activePath,
  openOrFocusWorkspace,
  pinWorkspace,
  unpinWorkspace,
  emptyText,
}: {
  title: string;
  items: SidebarWorkspace[];
  activePath: string;
  openOrFocusWorkspace: (path: string) => void;
  pinWorkspace: (path: string) => void;
  unpinWorkspace: (path: string) => void;
  emptyText: string;
}) {
  return (
    <section className="suite-side-section">
      <div className="suite-side-head">
        <p className="suite-side-label">{title}</p>
      </div>
      {!items.length ? (
        <p className="suite-side-empty">{emptyText}</p>
      ) : (
        <div className="suite-shortcut-list">
          {items.map((item) => {
            const isActive = item.path === activePath;
            return (
              <div key={`${title}-${item.path}`} className="suite-shortcut-row">
                <button
                  type="button"
                  className={isActive ? "suite-shortcut active" : "suite-shortcut"}
                  onClick={() => openOrFocusWorkspace(item.path)}
                  title={item.description}
                >
                  <span className="suite-shortcut-icon-shell">
                    <ShellIcon iconKey={item.iconKey} className="suite-shortcut-icon" />
                  </span>
                  <span className="suite-shortcut-copy">
                    <strong>{item.tabLabel}</strong>
                    <small>{item.workspaceKind === "document" ? "Document" : item.isOpen ? "Open workspace" : "Workspace"}</small>
                  </span>
                  {item.isOpen ? <span className="suite-shortcut-state">Open</span> : null}
                </button>
                <button
                  type="button"
                  className={item.pinned ? "suite-nav-pin active" : "suite-nav-pin"}
                  onClick={() => (item.pinned ? unpinWorkspace(item.path) : pinWorkspace(item.path))}
                  title={item.pinned ? "Unpin workspace" : "Pin workspace"}
                >
                  {item.pinned ? "Unpin" : "Pin"}
                </button>
              </div>
            );
          })}
        </div>
      )}
    </section>
  );
}

function ExplorerSidebar() {
  const {
    expandedGroups,
    toggleGroup,
    openOrFocusWorkspace,
    pinWorkspace,
    unpinWorkspace,
    tabs,
    tabEntries,
    favoriteWorkspaces,
    recentWorkspaces,
    activePath,
  } = useWorkspaceShell();
  const activeNavId = resolveNavSelection(activePath);
  const openWorkspaces = tabEntries
    .filter((entry) => entry.path !== activePath && !entry.pinned)
    .sort((a, b) => b.lastVisitedAt - a.lastVisitedAt)
    .slice(0, 5);
  const recentOnly = recentWorkspaces
    .filter((entry) => entry.path !== activePath && !entry.pinned && !entry.isOpen)
    .filter((entry, index, list) => list.findIndex((candidate) => candidate.path === entry.path) === index)
    .slice(0, 6);

  return (
    <aside className="suite-side">
      <NavLink to="/" className="suite-brand">
        <span className="suite-mark">
          <ShellIcon iconKey="overview" className="suite-brand-icon" size={22} />
        </span>
        <span>
          <strong>SIEM-Lite</strong>
          <small>Analyst console</small>
        </span>
      </NavLink>

      <div className="suite-side-section">
        <div className="suite-side-head">
          <p className="suite-side-label">Quick Access</p>
          <span className="suite-side-pill">{tabs.length} open</span>
        </div>
        <p className="suite-side-empty">Keep daily workspaces visible, collapse background noise, and reopen recent investigation context quickly.</p>
      </div>

      <WorkspaceShortcutSection
        title="Pinned"
        items={favoriteWorkspaces}
        activePath={activePath}
        openOrFocusWorkspace={openOrFocusWorkspace}
        pinWorkspace={pinWorkspace}
        unpinWorkspace={unpinWorkspace}
        emptyText="Pin important workspaces to keep them docked here."
      />

      <WorkspaceShortcutSection
        title="Open Now"
        items={openWorkspaces}
        activePath={activePath}
        openOrFocusWorkspace={openOrFocusWorkspace}
        pinWorkspace={pinWorkspace}
        unpinWorkspace={unpinWorkspace}
        emptyText="Open more workspaces to build a multitask shell."
      />

      <WorkspaceShortcutSection
        title="Recent"
        items={recentOnly}
        activePath={activePath}
        openOrFocusWorkspace={openOrFocusWorkspace}
        pinWorkspace={pinWorkspace}
        unpinWorkspace={unpinWorkspace}
        emptyText="Recent workspaces will appear here after navigation."
      />

      <div className="suite-side-section">
        <div className="suite-side-head">
          <p className="suite-side-label">Explorer</p>
          <span className="suite-side-pill">Grouped</span>
        </div>
        <nav className="suite-nav-tree">
          {SUITE_NAV_GROUPS.map((group) => {
            const isOpen = expandedGroups.includes(group.id);
            return (
              <section key={group.id} className="suite-nav-group">
                <button type="button" className="suite-nav-group-toggle" onClick={() => toggleGroup(group.id)}>
                  <span className="suite-nav-group-copy">
                    <strong>{group.label}</strong>
                    <small>{group.description}</small>
                  </span>
                  <span className="suite-nav-chevron">{isOpen ? "−" : "+"}</span>
                </button>
                {isOpen ? (
                  <div className="suite-nav-items">
                    {group.items.map((item) => {
                      const isPinned = tabs.some((tab) => tab.path === item.to && tab.pinned);
                      const isActive = activeNavId === item.id;
                      return (
                        <div key={item.id} className="suite-nav-item-row">
                          <button
                            type="button"
                            className={isActive ? "suite-nav-item active" : "suite-nav-item"}
                            onClick={() => openOrFocusWorkspace(item.to)}
                            title={item.description}
                          >
                            <span className="suite-nav-icon-shell">
                              <ShellIcon iconKey={item.iconKey} className="suite-nav-icon" />
                            </span>
                            <span className="suite-nav-copy">
                              <strong>{item.label}</strong>
                              <small>{item.description}</small>
                            </span>
                          </button>
                          <button
                            type="button"
                            className={isPinned ? "suite-nav-pin active" : "suite-nav-pin"}
                            onClick={() => (isPinned ? unpinWorkspace(item.to) : pinWorkspace(item.to))}
                            title={isPinned ? "Unpin workspace" : "Pin workspace"}
                          >
                            {isPinned ? "Unpin" : "Pin"}
                          </button>
                        </div>
                      );
                    })}
                  </div>
                ) : null}
              </section>
            );
          })}
        </nav>
      </div>
    </aside>
  );
}

function WorkspaceTabs() {
  const { tabs, activePath, openOrFocusWorkspace, closeWorkspace, pinWorkspace, unpinWorkspace } = useWorkspaceShell();
  const visibleTabs = tabs.filter((tab) => tab.path === activePath || tab.pinned || tab.workspaceKind === "document");
  const backgroundTabs = tabs.filter((tab) => !visibleTabs.some((entry) => entry.path === tab.path));
  const backgroundTabLabel = backgroundTabs.length === 1 ? "background tab" : "background tabs";
  const backgroundPreview = backgroundTabs.slice(0, 3).map((tab) => tab.tabLabel).join(", ");
  const overflowTitle = backgroundPreview
    ? `${backgroundTabs.length} ${backgroundTabLabel}: ${backgroundPreview}${backgroundTabs.length > 3 ? ", ..." : ""}`
    : `${backgroundTabs.length} ${backgroundTabLabel}`;

  const [backgroundMenuOpen, setBackgroundMenuOpen] = useState(false);

  useEffect(() => {
    if (!backgroundMenuOpen) return;
    const onKeyDown = (event: KeyboardEvent) => {
      if (event.key === "Escape") setBackgroundMenuOpen(false);
    };
    window.addEventListener("keydown", onKeyDown);
    return () => window.removeEventListener("keydown", onKeyDown);
  }, [backgroundMenuOpen]);

  useEffect(() => {
    if (!backgroundMenuOpen) return;
    const previous = document.body.style.overflow;
    document.body.style.overflow = "hidden";
    return () => {
      document.body.style.overflow = previous;
    };
  }, [backgroundMenuOpen]);

  useEffect(() => {
    if (!backgroundTabs.length) setBackgroundMenuOpen(false);
  }, [backgroundTabs.length]);

  return (
    <div className="workspace-tabs-shell">
      <div className="workspace-tabs">
        {visibleTabs.map((tab) => (
          <div
            key={tab.id}
            className={[
              "workspace-tab",
              tab.path === activePath ? "active" : "",
              tab.pinned ? "pinned" : "",
              tab.workspaceKind === "document" ? "document" : "workspace",
            ]
              .filter(Boolean)
              .join(" ")}
          >
            <button type="button" className="workspace-tab-main" onClick={() => openOrFocusWorkspace(tab.path)} title={tab.label}>
              <span className="workspace-tab-icon-shell">
                <ShellIcon iconKey={tab.iconKey} className="workspace-tab-icon" />
              </span>
              <span className="workspace-tab-copy">
                <span className="workspace-tab-label">{tab.tabLabel}</span>
                <span className="workspace-tab-meta">{tab.workspaceKind === "document" ? "Document" : "Workspace"}</span>
              </span>
            </button>
            <button
              type="button"
              className={tab.pinned ? "workspace-tab-pin active" : "workspace-tab-pin"}
              onClick={() => (tab.pinned ? unpinWorkspace(tab.path) : pinWorkspace(tab.path))}
              title={tab.pinned ? "Unpin tab" : "Pin tab"}
            >
              {tab.pinned ? "Unpin" : "Pin"}
            </button>
            {tab.closable && !tab.pinned ? (
              <button type="button" className="workspace-tab-close" onClick={() => closeWorkspace(tab.path)} title="Close tab">
                ×
              </button>
            ) : null}
          </div>
        ))}
        {backgroundTabs.length ? (
          <div className="workspace-tab-overflow-wrap">
            <button
              type="button"
              className={["workspace-tab", "workspace-tab-overflow", backgroundMenuOpen ? "open" : ""].filter(Boolean).join(" ")}
              title={overflowTitle}
              aria-expanded={backgroundMenuOpen}
              aria-haspopup="dialog"
              onClick={() => setBackgroundMenuOpen((open) => !open)}
            >
              <span className="workspace-tab-overflow-count">+{backgroundTabs.length}</span>
              <span className="workspace-tab-overflow-label">background</span>
            </button>
            {backgroundMenuOpen
              ? createPortal(
                  <div
                    className="workspace-bg-modal"
                    role="presentation"
                    onClick={() => setBackgroundMenuOpen(false)}
                  >
                    <div
                      className="workspace-bg-modal-dialog"
                      role="dialog"
                      aria-modal="true"
                      aria-labelledby="workspace-bg-modal-title"
                      onClick={(event) => event.stopPropagation()}
                    >
                      <div className="workspace-tab-overflow-menu-head">
                        <span id="workspace-bg-modal-title">Background workspaces</span>
                        <span className="workspace-tab-overflow-menu-count">{backgroundTabs.length}</span>
                      </div>
                      <ul className="workspace-tab-overflow-list" role="menu">
                        {backgroundTabs.map((tab) => (
                          <li key={tab.id} className="workspace-tab-overflow-item">
                            <button
                              type="button"
                              className="workspace-tab-overflow-open"
                              role="menuitem"
                              title={tab.label}
                              onClick={() => {
                                openOrFocusWorkspace(tab.path);
                                setBackgroundMenuOpen(false);
                              }}
                            >
                              <span className="workspace-tab-icon-shell">
                                <ShellIcon iconKey={tab.iconKey} className="workspace-tab-icon" />
                              </span>
                              <span className="workspace-tab-overflow-open-copy">
                                <span className="workspace-tab-overflow-open-label">{tab.tabLabel}</span>
                                <span className="workspace-tab-overflow-open-meta">
                                  {tab.workspaceKind === "document" ? "Document" : "Workspace"}
                                </span>
                              </span>
                            </button>
                            {tab.closable && !tab.pinned ? (
                              <button
                                type="button"
                                className="workspace-tab-overflow-remove"
                                title="Close tab"
                                aria-label={`Close ${tab.tabLabel}`}
                                onClick={() => {
                                  closeWorkspace(tab.path);
                                  if (backgroundTabs.length <= 1) setBackgroundMenuOpen(false);
                                }}
                              >
                                ×
                              </button>
                            ) : null}
                          </li>
                        ))}
                      </ul>
                    </div>
                  </div>,
                  document.body
                )
              : null}
          </div>
        ) : null}
      </div>
    </div>
  );
}

function AppShell({ actor }: { actor: string }) {
  return (
    <div className="suite-app">
      <ExplorerSidebar />
      <div className="suite-content">
        <SuiteTopbar />
        <WorkspaceTabs />
        <main className="suite-main">
          <Routes>
            <Route
              path="/"
              element={
                <WorkspaceRoute
                  title="SOC overview"
                  subtitle="Loading the native overview command surface and signal panels."
                >
                  <OverviewPage />
                </WorkspaceRoute>
              }
            />
            <Route
              path="/infrastructure"
              element={
                <WorkspaceRoute
                  title="Infrastructure"
                  subtitle="Loading the ECharts pilot screen and platform metrics."
                >
                  <InfrastructurePage />
                </WorkspaceRoute>
              }
            />
            <Route
              path="/operations"
              element={
                <WorkspaceRoute
                  title="Operations center"
                  subtitle="Loading the ECharts operations workspace and pipeline telemetry."
                >
                  <OperationsPage />
                </WorkspaceRoute>
              }
            />
            <Route
              path="/data-quality"
              element={
                <WorkspaceRoute
                  title="Data quality"
                  subtitle="Loading the ECharts trust layer and ingest quality metrics."
                >
                  <DataQualityPage />
                </WorkspaceRoute>
              }
            />
            <Route
              path="/validation"
              element={
                <WorkspaceRoute
                  title="Validation workspace"
                  subtitle="Loading trust checks, validation gauges and pipeline health signals."
                >
                  <ValidationPage />
                </WorkspaceRoute>
              }
            />
            <Route
              path="/dashboards"
              element={
                <WorkspaceRoute
                  title="Dashboards hub"
                  subtitle="Loading the analytics command center and surface catalog."
                >
                  <DashboardsPage />
                </WorkspaceRoute>
              }
            />
            <Route
              path="/alerts"
              element={
                <WorkspaceRoute
                  title="Alerts console"
                  subtitle="Loading the native alert inbox and analytics surfaces."
                >
                  <AlertsPage />
                </WorkspaceRoute>
              }
            />
            <Route
              path="/detections"
              element={
                <WorkspaceRoute
                  title="Detections console"
                  subtitle="Loading the engine pressure view, firing queue and rule telemetry."
                >
                  <DetectionsPage />
                </WorkspaceRoute>
              }
            />
            <Route
              path="/events"
              element={
                <WorkspaceRoute
                  title="Event search"
                  subtitle="Loading native event pivots, filters and entity context."
                >
                  <EventsPage />
                </WorkspaceRoute>
              }
            />
            <Route
              path="/cases"
              element={
                <WorkspaceRoute
                  title="Case operations"
                  subtitle="Loading the active response queue, ownership data and case actions."
                >
                  <CasesList />
                </WorkspaceRoute>
              }
            />
            <Route
              path="/cases/:id"
              element={
                <WorkspaceRoute
                  title="Case detail"
                  subtitle="Loading linked artifacts, timeline context and response workflow."
                >
                  <CaseDetail />
                </WorkspaceRoute>
              }
            />
            <Route
              path="/cases/:id/investigate"
              element={
                <WorkspaceRoute
                  title="Investigation workbench"
                  subtitle="Loading investigation pivots, linked evidence and analyst context."
                >
                  <InvestigationWorkbench />
                </WorkspaceRoute>
              }
            />
            <Route
              path="*"
              element={
                <WorkspaceRoute
                  title="Workspace handoff"
                  subtitle="Resolving the requested route and offering the nearest valid workspace."
                >
                  <UnknownRouteFallback />
                </WorkspaceRoute>
              }
            />
          </Routes>
        </main>
      </div>
      <CommandPalette actor={actor} />
    </div>
  );
}

export default function App() {
  const { actor } = useActorState();

  return (
    <SuiteCommandProvider>
      <WorkspaceShellProvider>
        <AppShell actor={actor} />
      </WorkspaceShellProvider>
    </SuiteCommandProvider>
  );
}
