import { createContext, useContext, useEffect, useMemo, useState, type ReactNode } from "react";

export type SuitePageCommand = {
  id: string;
  title: string;
  subtitle: string;
  section?: string;
  keywords?: string;
  priority?: number;
  href?: string;
  external?: boolean;
  run?: () => void | Promise<void>;
};

type SuiteCommandContextValue = {
  pageCommands: SuitePageCommand[];
  setPageCommands: (commands: SuitePageCommand[]) => void;
};

const SuiteCommandContext = createContext<SuiteCommandContextValue | null>(null);

export function SuiteCommandProvider({ children }: { children: ReactNode }) {
  const [pageCommands, setPageCommands] = useState<SuitePageCommand[]>([]);
  const value = useMemo(
    () => ({
      pageCommands,
      setPageCommands,
    }),
    [pageCommands]
  );

  return <SuiteCommandContext.Provider value={value}>{children}</SuiteCommandContext.Provider>;
}

export function useSuiteCommandContext() {
  const context = useContext(SuiteCommandContext);
  if (!context) {
    throw new Error("useSuiteCommandContext must be used inside SuiteCommandProvider");
  }
  return context;
}

export function usePublishPageCommands(commands: SuitePageCommand[]) {
  const { setPageCommands } = useSuiteCommandContext();

  useEffect(() => {
    setPageCommands(commands);
    return () => setPageCommands([]);
  }, [commands, setPageCommands]);
}
