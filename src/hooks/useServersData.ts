import { useState, useEffect, useCallback } from "react";
import {
  getAllServers,
  getServerById,
  createServer,
  updateServer,
  deleteServer,
} from "@/api/serverMethods";
import { IServer, IServerPrimitive } from "@/models/server";

import { toast } from "react-hot-toast";

export function useServersData() {
  const [servers, setServers] = useState<IServer[]>([]);
  const [selectedServer, setSelectedServer] = useState<IServer | null>(null);
  const [isLoading, setIsLoading] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const fetchServers = useCallback(async () => {
    setIsLoading(true);
    setError(null);
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
    setIsLoading(true);
    setError(null);
    try {
      const newSrv = await createServer(data);
      setServers((prev) => [...prev, newSrv]);
      toast.success(`Servidor "${newSrv.name}" adicionado!`);
      return newSrv;
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg);
      toast.error(msg);
    } finally {
      setIsLoading(false);
    }
  }, []);

  const getServer = useCallback(async (id: number) => {
    setIsLoading(true);
    setError(null);
    try {
      const srv = await getServerById(id);
      return srv;
    } catch (err) {
      const msg = err instanceof Error ? err.message : String(err);
      setError(msg);
      throw new Error(msg);
    } finally {
      setIsLoading(false);
    }
  }, []);

  const editServer = useCallback(
    async (id: number, data: IServerPrimitive) => {
      setIsLoading(true);
      setError(null);
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
        const msg = err instanceof Error ? err.message : String(err);
        setError(msg);
        toast.error(msg);
      } finally {
        setIsLoading(false);
      }
    },
    [selectedServer]
  );

  const removeServer = useCallback(
    async (id: number) => {
      setIsLoading(true);
      setError(null);
      try {
        await deleteServer(id);
        setServers((prev) => prev.filter((s) => s.id !== id));
        if (selectedServer?.id === id) {
          setSelectedServer(null);
        }

        toast.success(`Servidor removido com sucesso!`);
      } catch (err) {
        const msg = err instanceof Error ? err.message : String(err);
        setError(msg);
        toast.error(msg);
      } finally {
        setIsLoading(false);
      }
    },
    [selectedServer]
  );

  const selectServer = useCallback(
    (id: number | null) => {
      if (id === null) {
        setSelectedServer(null);
      } else {
        const found = servers.find((s) => s.id === id) || null;
        setSelectedServer(found);
      }
    },
    [servers]
  );

  useEffect(() => {
    fetchServers();
  }, [fetchServers]);

  return {
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
}
