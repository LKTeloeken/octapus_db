import { createContext, ReactNode, useContext } from "react";
import { useDbStructure as _useDbStructure } from "@/shared/hooks/use-db-structure";

import type { IUseDbStructureParams } from "@/shared/hooks/use-db-structure";

export interface DbStructureProviderReturn
  extends ReturnType<typeof _useDbStructure> {}

const ServersContext = createContext<DbStructureProviderReturn | null>(null);

export function DbStructureProvider({
  children,
  setServers,
}: {
  children: ReactNode;
} & IUseDbStructureParams) {
  const value = _useDbStructure({ setServers });
  return (
    <ServersContext.Provider value={value}>{children}</ServersContext.Provider>
  );
}

export function useDbStructure() {
  const context = useContext(ServersContext);
  if (!context) {
    throw new Error("useServers must be used within a ServersProvider");
  }
  return context;
}
