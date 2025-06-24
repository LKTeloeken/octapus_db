import { useState, useEffect, useCallback } from "react";
import {
  getAllServers,
  getServerById,
  createServer,
  updateServer,
  deleteServer,
} from "@/api/serverMethods";
import {
  connectToPostgreServer,
  getPostgreDatabases,
  getPostgreSchemas,
} from "@/api/postgreMethods";
import { IServer, IServerPrimitive } from "@/models/server";

import { toast } from "react-hot-toast";

export function useServersData() {
  const [servers, setServers] = useState<IServer[]>([]);
  const [selectedServer, setSelectedServer] = useState<IServer | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [isConnecting, setIsConnecting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const initLoadingState = () => {
    setIsLoading(false);
    setIsConnecting(false);
    setError(null);
  };

  const handleError = (err: unknown) => {
    const msg = err instanceof Error ? err.message : String(err);
    setError(msg);
    toast.error(msg);
  };

  const fetchServers = useCallback(async () => {
    initLoadingState();

    try {
      const list = await getAllServers();
      setServers(list);
    } catch (err) {
      setError(err instanceof Error ? err.message : String(err));
    } finally {
      setIsLoading(false);
    }
  }, []);

  const addServer = useCallback(async (data: IServerPrimitive) => {
    initLoadingState();

    try {
      const newSrv = await createServer(data);
      setServers((prev) => [...prev, newSrv]);
      toast.success(`Servidor "${newSrv.name}" adicionado!`);
      return newSrv;
    } catch (err) {
      handleError(err);
    } finally {
      setIsLoading(false);
    }
  }, []);

  const getServer = useCallback(async (id: number) => {
    initLoadingState();

    try {
      const srv = await getServerById(id);
      return srv;
    } catch (err) {
      handleError(err);
    } finally {
      setIsLoading(false);
    }
  }, []);

  const editServer = useCallback(
    async (id: number, data: IServerPrimitive) => {
      initLoadingState();

      try {
        const updated = await updateServer(
          id,
          data.name,
          data.host,
          data.port,
          data.username,
          data.password,
          data.default_database
        );
        setServers((prev) => prev.map((s) => (s.id === id ? updated : s)));
        if (selectedServer?.id === id) {
          setSelectedServer(updated);
        }

        toast.success(`Servidor "${updated.name}" atualizado!`);
        return updated;
      } catch (err) {
        handleError(err);
      } finally {
        setIsLoading(false);
      }
    },
    [selectedServer]
  );

  const removeServer = useCallback(
    async (id: number) => {
      initLoadingState();

      try {
        await deleteServer(id);
        setServers((prev) => prev.filter((s) => s.id !== id));
        if (selectedServer?.id === id) {
          setSelectedServer(null);
        }

        toast.success(`Servidor removido com sucesso!`);
      } catch (err) {
        handleError(err);
      } finally {
        setIsLoading(false);
      }
    },
    [selectedServer]
  );

  const connectToServer = useCallback(async (server: IServer) => {
    if (server.isConnected) return true;
    if (isConnecting) return;

    initLoadingState();

    try {
      const hasConnected = await connectToPostgreServer(
        server.id,
        server.default_database
      );

      setServers((prev) =>
        prev.map((s) =>
          s.id === server.id ? { ...s, isConnected: hasConnected } : s
        )
      );

      toast.success(`Conectado ao servidor "${server.name}"!`);

      try {
        const serverDatabases = await getPostgreDatabases(server);

        if (!serverDatabases || !serverDatabases?.length) return hasConnected;

        setServers((prev) =>
          prev.map((s) =>
            s.id === server.id ? { ...s, databases: serverDatabases } : s
          )
        );

        toast.success(`Databases do servidor "${server.name}" carregadas!`);

        return hasConnected;
      } catch (error) {
        handleError(error);
      }

      return hasConnected;
    } catch (err) {
      handleError(err);
    } finally {
      setIsConnecting(false);
    }
  }, []);

  const getDatabaseSchemas = useCallback(
    async (serverId: number, databaseName?: string) => {
      initLoadingState();

      try {
        const schemas = await getPostgreSchemas(serverId, databaseName);

        if (!schemas || !schemas.length) {
          toast.error("Nenhum schema encontrado para este servidor.");
          return [];
        }

        setServers((prev) =>
          prev.map((s) =>
            s.id === serverId
              ? {
                  ...s,
                  databases: s.databases?.map((db) =>
                    db.name === databaseName ? { ...db, schemas } : db
                  ),
                }
              : s
          )
        );

        toast.success(
          `Schemas do database "${databaseName}" carregados com sucesso!`
        );

        return schemas;
      } catch (err) {
        handleError(err);
      } finally {
        setIsLoading(false);
      }
    },
    []
  );

  useEffect(() => {
    fetchServers();
  }, [fetchServers]);

  return {
    servers,
    selectedServer,
    isLoading,
    isConnecting,
    error,
    fetchServers,
    addServer,
    getServer,
    editServer,
    removeServer,
    connectToServer,
    getDatabaseSchemas,
  };
}
