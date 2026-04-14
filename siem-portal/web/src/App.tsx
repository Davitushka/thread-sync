import { NavLink, Route, Routes } from "react-router-dom";
import { SuiteTopbar, useActorState } from "./components/PageLayout";
import CommandPalette from "./components/CommandPalette";
import ShellIcon from "./components/ShellIcon";
import { SuiteCommandProvider } from "./components/SuiteCommandContext";
import { WorkspaceShellProvider, useWorkspaceShell } from "./components/WorkspaceShellContext";
import OverviewPage from "./pages/OverviewPage";
import InfrastructurePage from "./pages/InfrastructurePage";
import OperationsPage from "./pages/OperationsPage";
import DataQualityPage from "./pages/DataQualityPage";
import ValidationPage from "./pages/ValidationPage";
import AlertsPage from "./pages/AlertsPage";
import DetectionsPage from "./pages/DetectionsPage";
import DashboardsPage from "./pages/DashboardsPage";
import EventsPage from "./pages/EventsPage";
import CasesList from "./pages/CasesList";
import CaseDetail from "./pages/CaseDetail";
import InvestigationWorkbench from "./pages/InvestigationWorkbench";
import { SUITE_NAV_GROUPS, resolveNavSelection } from "./suite-meta";

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
          <div className="workspace-tab workspace-tab-overflow" title={`${backgroundTabs.length} additional workspaces are open`}>
            <span className="workspace-tab-overflow-count">+{backgroundTabs.length}</span>
            <span className="workspace-tab-overflow-label">more open</span>
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
            <Route path="/" element={<OverviewPage />} />
            <Route path="/infrastructure" element={<InfrastructurePage />} />
            <Route path="/operations" element={<OperationsPage />} />
            <Route path="/data-quality" element={<DataQualityPage />} />
            <Route path="/validation" element={<ValidationPage />} />
            <Route path="/dashboards" element={<DashboardsPage />} />
            <Route path="/alerts" element={<AlertsPage />} />
            <Route path="/detections" element={<DetectionsPage />} />
            <Route path="/events" element={<EventsPage />} />
            <Route path="/cases" element={<CasesList />} />
            <Route path="/cases/:id" element={<CaseDetail />} />
            <Route path="/cases/:id/investigate" element={<InvestigationWorkbench />} />
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
