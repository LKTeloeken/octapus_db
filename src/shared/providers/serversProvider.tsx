import React, { createContext, ReactNode, useContext } from "react";
import { useServersData } from "@/shared/hooks/use-server-data";

const ServersContext = createContext<ReturnType<typeof useServersData> | null>(
  null
);

export function ServersProvider({ children }: { children: ReactNode }) {
  const value = useServersData();
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
