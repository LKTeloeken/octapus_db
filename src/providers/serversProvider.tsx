import React, {
  createContext,
  useContext,
  useState,
  useEffect,
  ReactNode,
} from "react";
import {
  createServer,
  getAllServers,
  getServerById,
  updateServer,
  deleteServer,
} from "@/api/postgreMethods";
import { IPostgreServer, IPostgreServerPrimitive } from "@/models/postgreDb";

interface ServersContextType {
  servers: IPostgreServer[];
  selectedServer: IPostgreServer | null;
  isLoading: boolean;
  error: string | null;
  fetchServers: () => Promise<void>;
  addServer: (server: IPostgreServerPrimitive) => Promise<IPostgreServer>;
  getServer: (id: number) => Promise<IPostgreServer>;
  editServer: (
    id: number,
    server: IPostgreServerPrimitive
  ) => Promise<IPostgreServer>;
  removeServer: (id: number) => Promise<void>;
  selectServer: (id: number | null) => void;
}

const ServersContext = createContext<ServersContextType | undefined>(undefined);

export function useServers() {
  const context = useContext(ServersContext);
  if (context === undefined) {
    throw new Error("useServers must be used within a ServersProvider");
  }
  return context;
}

interface ServersProviderProps {
  children: ReactNode;
}

export function ServersProvider({ children }: ServersProviderProps) {
  const [servers, setServers] = useState<IPostgreServer[]>([]);
  const [selectedServer, setSelectedServer] = useState<IPostgreServer | null>(
    null
  );
  const [isLoading, setIsLoading] = useState<boolean>(false);
  const [error, setError] = useState<string | null>(null);

  const fetchServers = async () => {
    setIsLoading(true);
    setError(null);
    try {
      const serversList = await getAllServers();
      setServers(serversList);
    } catch (err) {
      setError(
        `Failed to fetch servers: ${
          err instanceof Error ? err.message : String(err)
        }`
      );
    } finally {
      setIsLoading(false);
    }
  };

  const addServer = async (
    server: IPostgreServerPrimitive
  ): Promise<IPostgreServer> => {
    setIsLoading(true);
    setError(null);
    try {
      const newServer = await createServer(server);
      setServers((prev) => [...prev, newServer]);
      return newServer;
    } catch (err) {
      const errorMessage = `Failed to create server: ${
        err instanceof Error ? err.message : String(err)
      }`;
      setError(errorMessage);
      throw new Error(errorMessage);
    } finally {
      setIsLoading(false);
    }
  };

  const getServer = async (id: number): Promise<IPostgreServer> => {
    setIsLoading(true);
    setError(null);
    try {
      return await getServerById(id);
    } catch (err) {
      const errorMessage = `Failed to get server: ${
        err instanceof Error ? err.message : String(err)
      }`;
      setError(errorMessage);
      throw new Error(errorMessage);
    } finally {
      setIsLoading(false);
    }
  };

  const editServer = async (
    id: number,
    serverData: IPostgreServerPrimitive
  ): Promise<IPostgreServer> => {
    setIsLoading(true);
    setError(null);
    try {
      const updatedServer = await updateServer(
        id,
        serverData.name as string,
        serverData.host as string,
        serverData.port,
        serverData.username as string,
        serverData.password as string
      );
      setServers((prev) =>
        prev.map((server) => (server.id === id ? updatedServer : server))
      );
      if (selectedServer?.id === id) {
        setSelectedServer(updatedServer);
      }
      return updatedServer;
    } catch (err) {
      const errorMessage = `Failed to update server: ${
        err instanceof Error ? err.message : String(err)
      }`;
      setError(errorMessage);
      throw new Error(errorMessage);
    } finally {
      setIsLoading(false);
    }
  };

  const removeServer = async (id: number): Promise<void> => {
    setIsLoading(true);
    setError(null);
    try {
      await deleteServer(id);
      setServers((prev) => prev.filter((server) => server.id !== id));
      if (selectedServer?.id === id) {
        setSelectedServer(null);
      }
    } catch (err) {
      const errorMessage = `Failed to delete server: ${
        err instanceof Error ? err.message : String(err)
      }`;
      setError(errorMessage);
      throw new Error(errorMessage);
    } finally {
      setIsLoading(false);
    }
  };

  const selectServer = (id: number | null) => {
    if (id === null) {
      setSelectedServer(null);
      return;
    }
    const server = servers.find((s) => s.id === id);
    setSelectedServer(server || null);
  };

  useEffect(() => {
    fetchServers();
  }, []);

  const value: ServersContextType = {
    servers,
    selectedServer,
    isLoading,
    error,
    fetchServers,
    addServer,
    getServer,
    editServer,
    removeServer,
    selectServer,
  };

  return (
    <ServersContext.Provider value={value}>{children}</ServersContext.Provider>
  );
}
