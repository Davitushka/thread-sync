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
import { useLocation, useNavigate } from "react-router-dom";
import {
  DEFAULT_EXPANDED_GROUPS,
  DEFAULT_PINNED_PATHS,
  DEFAULT_WORKSPACE_PATHS,
  resolveWorkspaceMeta,
  type ResolvedWorkspaceMeta,
  type ShellGroupId,
  type WorkspaceMetaOverride,
} from "../suite-meta";

type WorkspaceTab = {
  id: string;
  path: string;
  label: string;
  tabLabel: string;
  iconKey: string;
  groupId: ShellGroupId;
  closable: boolean;
  pinned: boolean;
  workspaceKind: "workspace" | "document";
  lastVisitedAt: number;
};

type WorkspaceEntry = ResolvedWorkspaceMeta & {
  tabId: string;
  pinned: boolean;
  isOpen: boolean;
  lastVisitedAt: number;
};

type PersistedShellState = {
  version?: number;
  openPaths: string[];
  pinnedPaths: string[];
  expandedGroups: ShellGroupId[];
  recentPaths: string[];
};

type WorkspaceShellContextValue = {
  tabs: WorkspaceTab[];
  tabEntries: WorkspaceEntry[];
  favoriteWorkspaces: WorkspaceEntry[];
  recentWorkspaces: WorkspaceEntry[];
  activePath: string;
  activeTabId: string | null;
  expandedGroups: ShellGroupId[];
  recentPaths: string[];
  activeWorkspace: ResolvedWorkspaceMeta;
  openWorkspace: (path: string, override?: WorkspaceMetaOverride) => void;
  focusWorkspace: (path: string) => void;
  openOrFocusWorkspace: (path: string, override?: WorkspaceMetaOverride) => void;
  closeWorkspace: (path: string) => void;
  pinWorkspace: (path: string) => void;
  unpinWorkspace: (path: string) => void;
  reopenRecentWorkspace: (path: string) => void;
  toggleGroup: (groupId: ShellGroupId) => void;
  setExpandedGroups: (groups: ShellGroupId[]) => void;
  updateWorkspaceMeta: (path: string, override?: WorkspaceMetaOverride) => void;
};

const STORAGE_KEY = "suite_workspace_shell";
const STORAGE_VERSION = 3;
const RECENT_LIMIT = 12;

const WorkspaceShellContext = createContext<WorkspaceShellContextValue | null>(null);

function now() {
  return Date.now();
}

function unique<T>(values: T[]) {
  return Array.from(new Set(values));
}

function readPersistedState(): PersistedShellState {
  try {
    const raw = localStorage.getItem(STORAGE_KEY);
    if (!raw) {
      return {
        openPaths: DEFAULT_WORKSPACE_PATHS,
        pinnedPaths: DEFAULT_PINNED_PATHS,
        expandedGroups: DEFAULT_EXPANDED_GROUPS,
        recentPaths: [],
      };
    }
    const parsed = JSON.parse(raw) as Partial<PersistedShellState>;
    if ((parsed.version ?? 0) < STORAGE_VERSION) {
      return {
        version: STORAGE_VERSION,
        openPaths: DEFAULT_WORKSPACE_PATHS,
        pinnedPaths: DEFAULT_PINNED_PATHS,
        expandedGroups: DEFAULT_EXPANDED_GROUPS,
        recentPaths: unique(parsed.recentPaths ?? []).slice(0, RECENT_LIMIT),
      };
    }
    return {
      version: STORAGE_VERSION,
      openPaths: unique(parsed.openPaths ?? DEFAULT_WORKSPACE_PATHS),
      pinnedPaths: unique(parsed.pinnedPaths ?? DEFAULT_PINNED_PATHS),
      expandedGroups: unique((parsed.expandedGroups as ShellGroupId[] | undefined) ?? DEFAULT_EXPANDED_GROUPS),
      recentPaths: unique(parsed.recentPaths ?? []).slice(0, RECENT_LIMIT),
    };
  } catch {
    return {
      version: STORAGE_VERSION,
      openPaths: DEFAULT_WORKSPACE_PATHS,
      pinnedPaths: DEFAULT_PINNED_PATHS,
      expandedGroups: DEFAULT_EXPANDED_GROUPS,
      recentPaths: [],
    };
  }
}

function buildTab(path: string, pinnedPaths: string[]): WorkspaceTab {
  const meta = resolveWorkspaceMeta(path);
  return {
    id: `${meta.id}:${path}`,
    path,
    label: meta.label,
    tabLabel: meta.tabLabel,
    iconKey: meta.iconKey,
    groupId: meta.groupId,
    closable: meta.closable,
    pinned: pinnedPaths.includes(path),
    workspaceKind: meta.workspaceKind,
    lastVisitedAt: now(),
  };
}

export function WorkspaceShellProvider({ children }: { children: ReactNode }) {
  const location = useLocation();
  const navigate = useNavigate();
  const initial = useRef(readPersistedState());
  const [pinnedPaths, setPinnedPaths] = useState<string[]>(() => initial.current.pinnedPaths);
  const [expandedGroups, setExpandedGroups] = useState<ShellGroupId[]>(() => initial.current.expandedGroups);
  const [recentPaths, setRecentPaths] = useState<string[]>(() => initial.current.recentPaths);
  const [tabs, setTabs] = useState<WorkspaceTab[]>(() => {
    const basePaths = unique([...initial.current.openPaths, location.pathname]);
    return basePaths.map((path) => buildTab(path, initial.current.pinnedPaths));
  });
  const metaOverridesRef = useRef<Record<string, WorkspaceMetaOverride>>({});

  const touchRecentPath = useCallback((path: string) => {
    setRecentPaths((current) => [path, ...current.filter((item) => item !== path)].slice(0, RECENT_LIMIT));
  }, []);

  const updateWorkspaceMeta = useCallback((path: string, override?: WorkspaceMetaOverride) => {
    if (!override) {
      delete metaOverridesRef.current[path];
      return;
    }
    metaOverridesRef.current[path] = {
      ...metaOverridesRef.current[path],
      ...override,
    };
    setTabs((current) =>
      current.map((tab) => {
        if (tab.path !== path) return tab;
        const meta = resolveWorkspaceMeta(path, metaOverridesRef.current[path]);
        return {
          ...tab,
          label: meta.label,
          tabLabel: meta.tabLabel,
          iconKey: meta.iconKey,
          groupId: meta.groupId,
          closable: meta.closable,
        };
      })
    );
  }, []);

  const ensureTab = useCallback(
    (path: string, override?: WorkspaceMetaOverride) => {
      if (override) {
        metaOverridesRef.current[path] = {
          ...metaOverridesRef.current[path],
          ...override,
        };
      }
      setTabs((current) => {
        const index = current.findIndex((tab) => tab.path === path);
        const meta = resolveWorkspaceMeta(path, metaOverridesRef.current[path]);
        if (index >= 0) {
          const next = [...current];
          next[index] = {
            ...next[index],
            label: meta.label,
            tabLabel: meta.tabLabel,
            iconKey: meta.iconKey,
            groupId: meta.groupId,
            closable: meta.closable,
            pinned: pinnedPaths.includes(path),
            lastVisitedAt: now(),
          };
          return next;
        }
        return [
          ...current,
          {
            id: `${meta.id}:${path}`,
            path,
            label: meta.label,
            tabLabel: meta.tabLabel,
            iconKey: meta.iconKey,
            groupId: meta.groupId,
            closable: meta.closable,
            pinned: pinnedPaths.includes(path),
            workspaceKind: meta.workspaceKind,
            lastVisitedAt: now(),
          },
        ];
      });
    },
    [pinnedPaths]
  );

  const focusWorkspace = useCallback(
    (path: string) => {
      ensureTab(path);
      if (location.pathname !== path) {
        navigate(path);
      }
      touchRecentPath(path);
    },
    [ensureTab, location.pathname, navigate, touchRecentPath]
  );

  const openWorkspace = useCallback(
    (path: string, override?: WorkspaceMetaOverride) => {
      ensureTab(path, override);
      if (location.pathname !== path) {
        navigate(path);
      }
      touchRecentPath(path);
    },
    [ensureTab, location.pathname, navigate, touchRecentPath]
  );

  const openOrFocusWorkspace = useCallback(
    (path: string, override?: WorkspaceMetaOverride) => {
      ensureTab(path, override);
      if (location.pathname !== path) {
        navigate(path);
      }
      touchRecentPath(path);
    },
    [ensureTab, location.pathname, navigate, touchRecentPath]
  );

  const closeWorkspace = useCallback(
    (path: string) => {
      setTabs((current) => {
        const tab = current.find((item) => item.path === path);
        if (!tab || tab.pinned || !tab.closable) {
          return current;
        }
        const next = current.filter((item) => item.path !== path);
        if (!next.length) {
          const fallback = buildTab("/", pinnedPaths);
          return [fallback];
        }
        if (location.pathname === path) {
          const fallback = next[next.length - 1];
          queueMicrotask(() => navigate(fallback.path));
        }
        return next;
      });
      touchRecentPath(path);
    },
    [location.pathname, navigate, pinnedPaths, touchRecentPath]
  );

  const pinWorkspace = useCallback((path: string) => {
    setPinnedPaths((current) => unique([...current, path]));
    ensureTab(path);
  }, [ensureTab]);

  const unpinWorkspace = useCallback((path: string) => {
    setPinnedPaths((current) => current.filter((item) => item !== path));
  }, []);

  const reopenRecentWorkspace = useCallback(
    (path: string) => {
      openOrFocusWorkspace(path);
    },
    [openOrFocusWorkspace]
  );

  const toggleGroup = useCallback((groupId: ShellGroupId) => {
    setExpandedGroups((current) =>
      current.includes(groupId) ? current.filter((item) => item !== groupId) : [...current, groupId]
    );
  }, []);

  useEffect(() => {
    ensureTab(location.pathname);
    touchRecentPath(location.pathname);
  }, [ensureTab, location.pathname, touchRecentPath]);

  useEffect(() => {
    setTabs((current) =>
      current.map((tab) => ({
        ...tab,
        pinned: pinnedPaths.includes(tab.path),
      }))
    );
  }, [pinnedPaths]);

  useEffect(() => {
    const persisted: PersistedShellState = {
      version: STORAGE_VERSION,
      openPaths: unique(tabs.map((tab) => tab.path)),
      pinnedPaths,
      expandedGroups,
      recentPaths,
    };
    localStorage.setItem(STORAGE_KEY, JSON.stringify(persisted));
  }, [tabs, pinnedPaths, expandedGroups, recentPaths]);

  const activeTab = tabs.find((tab) => tab.path === location.pathname) ?? tabs[0] ?? null;
  const activeWorkspace = resolveWorkspaceMeta(location.pathname, metaOverridesRef.current[location.pathname]);

  const tabEntries = useMemo<WorkspaceEntry[]>(
    () =>
      tabs.map((tab) => {
        const meta = resolveWorkspaceMeta(tab.path, metaOverridesRef.current[tab.path]);
        return {
          ...meta,
          tabId: tab.id,
          path: tab.path,
          pinned: tab.pinned,
          isOpen: true,
          lastVisitedAt: tab.lastVisitedAt,
        };
      }),
    [tabs]
  );

  const favoriteWorkspaces = useMemo<WorkspaceEntry[]>(
    () =>
      unique(pinnedPaths)
        .map((path) => {
          const existing = tabEntries.find((entry) => entry.path === path);
          if (existing) return existing;
          const meta = resolveWorkspaceMeta(path, metaOverridesRef.current[path]);
          return {
            ...meta,
            tabId: `${meta.id}:${path}`,
            path,
            pinned: true,
            isOpen: false,
            lastVisitedAt: 0,
          };
        })
        .sort((a, b) => a.tabLabel.localeCompare(b.tabLabel)),
    [pinnedPaths, tabEntries]
  );

  const recentWorkspaces = useMemo<WorkspaceEntry[]>(
    () =>
      unique(recentPaths)
        .map((path) => {
          const existing = tabEntries.find((entry) => entry.path === path);
          if (existing) return existing;
          const meta = resolveWorkspaceMeta(path, metaOverridesRef.current[path]);
          return {
            ...meta,
            tabId: `${meta.id}:${path}`,
            path,
            pinned: pinnedPaths.includes(path),
            isOpen: false,
            lastVisitedAt: 0,
          };
        }),
    [recentPaths, tabEntries, pinnedPaths]
  );

  const value = useMemo<WorkspaceShellContextValue>(
    () => ({
      tabs,
      tabEntries,
      favoriteWorkspaces,
      recentWorkspaces,
      activePath: location.pathname,
      activeTabId: activeTab?.id ?? null,
      expandedGroups,
      recentPaths,
      activeWorkspace,
      openWorkspace,
      focusWorkspace,
      openOrFocusWorkspace,
      closeWorkspace,
      pinWorkspace,
      unpinWorkspace,
      reopenRecentWorkspace,
      toggleGroup,
      setExpandedGroups,
      updateWorkspaceMeta,
    }),
    [
      tabs,
      tabEntries,
      favoriteWorkspaces,
      recentWorkspaces,
      location.pathname,
      activeTab,
      expandedGroups,
      recentPaths,
      activeWorkspace,
      openWorkspace,
      focusWorkspace,
      openOrFocusWorkspace,
      closeWorkspace,
      pinWorkspace,
      unpinWorkspace,
      reopenRecentWorkspace,
      toggleGroup,
      updateWorkspaceMeta,
    ]
  );

  return <WorkspaceShellContext.Provider value={value}>{children}</WorkspaceShellContext.Provider>;
}

export function useWorkspaceShell() {
  const context = useContext(WorkspaceShellContext);
  if (!context) {
    throw new Error("useWorkspaceShell must be used inside WorkspaceShellProvider");
  }
  return context;
}
