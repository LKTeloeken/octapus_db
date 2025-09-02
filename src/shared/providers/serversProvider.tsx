import { type ReactNode, useContext, createContext } from "react";
import { useServerConnections } from "@/shared/hooks/use-server-data";

export interface ServersProviderReturn
  extends ReturnType<typeof useServerConnections> {}

const ServersContext = createContext<ServersProviderReturn | null>(null);

export function ServersProvider({ children }: { children: ReactNode }) {
  const value = useServerConnections();
  return (
    <ServersContext.Provider value={value}>{children}</ServersContext.Provider>
  );
}

export function useServers() {
  const context = useContext(ServersContext);
  if (!context) {
    throw new Error("useServers must be used within a ServersProvider");
  }
  return context;
}
