import { type ReactNode, useContext, createContext } from "react";
import { useQueryTabs } from "@/shared/hooks/use-query-tabs";

export interface ServersProviderReturn
  extends ReturnType<typeof useQueryTabs> {}

const TabsContext = createContext<ServersProviderReturn | null>(null);

export function TabsProvider({ children }: { children: ReactNode }) {
  const value = useQueryTabs();
  return <TabsContext.Provider value={value}>{children}</TabsContext.Provider>;
}

export function useTabs() {
  const context = useContext(TabsContext);
  if (!context) {
    throw new Error("useServers must be used within a ServersProvider");
  }
  return context;
}
