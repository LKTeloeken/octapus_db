import { useState, useEffect, useCallback, useMemo } from "react";
import { toast } from "react-hot-toast";
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
} from "@/api/postgreMethods";
import { createRunner } from "@/shared/utils/asyncRunner";
import {
  key,
  createTreeActions,
  createLoadChildren,
} from "@/shared/utils/serverTree";

import type { IServer, IServerPrimitive } from "@/shared/models/server";
import type { ITreeNode } from "@/shared/models/tree";

/**
 * Hook — Responsável por:
 *  - Buscar/CRUD dos servidores cadastrados
 *  - Conectar ao servidor
 *  - Gerenciar o carregamento de *databases* do servidor
 *
 * Observação: Este hook mantém seu próprio estado de árvore (`servers`).
 * Você pode passar `autoFetch=false` para não carregar automaticamente.
 */
export function useServerConnections(opts?: { autoFetch?: boolean }) {
  const autoFetch = opts?.autoFetch ?? true;

  const [servers, setServers] = useState<Record<string, ITreeNode>>({});
  const [isLoading, setIsLoading] = useState(false);
  const [isConnecting, setIsConnecting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = useMemo(
    () =>
      createRunner({
        setIsLoading,
        setIsConnecting,
        setError,
      }),
    []
  );

  const tree = useMemo(() => createTreeActions(setServers), []);
  const loadChildren = useMemo(
    () => createLoadChildren(run, tree),
    [run, tree]
  );

  // =====================
  // CRUD Servidores
  // =====================

  const fetchServers = useCallback(async () => {
    await run({
      task: async () => {
        const serverList = await getAllServers();
        const treeNodes: Record<string, ITreeNode> = {};

        serverList.forEach((srv) => {
          const srvKey = key("server", srv.id);
          treeNodes[srvKey] = { name: srv.name, children: [], data: srv };
        });

        setServers(treeNodes);
        return serverList;
      },
    });
  }, [run]);

  const addServer = useCallback(
    async (data: IServerPrimitive) => {
      return run({
        task: async () => {
          const newSrv = await createServer(data);
          tree.upsertNode(key("server", newSrv.id), newSrv.name, newSrv);
          toast.success(`Servidor "${newSrv.name}" adicionado!`);
          return newSrv;
        },
      });
    },
    [run, tree]
  );

  const getServer = useCallback(
    async (id: number) => {
      return run({
        task: () => getServerById(id),
      });
    },
    [run]
  );

  const editServer = useCallback(
    async (id: number, data: IServerPrimitive) => {
      return run({
        task: async () => {
          const updated = await updateServer(
            id,
            data.name,
            data.host,
            data.port,
            data.username,
            data.password,
            data.default_database
          );
          tree.upsertNode(key("server", id), updated.name, updated);
          toast.success(`Servidor "${updated.name}" atualizado!`);
          return updated;
        },
      });
    },
    [run, tree]
  );

  const removeServer = useCallback(
    async (id: number) => {
      return run({
        task: async () => {
          await deleteServer(id);
          tree.removeSubtree(key("server", id));
          toast.success("Servidor removido com sucesso!");
          return true;
        },
      });
    },
    [run, tree]
  );

  // =====================
  // Conexão e carregamentos (somente databases)
  // =====================

  const connectToServer = useCallback(
    async (server: IServer) => {
      if (server.isConnected) return true;
      if (isConnecting) return; // evita corrida simples

      return run<boolean>({
        kind: "connect",
        task: async () => {
          const hasConnected = await connectToPostgreServer(
            server.id,
            server.default_database
          );

          tree.upsertNode(key("server", server.id), server.name, {
            isConnected: hasConnected,
            default_database: server.default_database,
          });

          toast.success(`Conectado ao servidor "${server.name}"!`);

          // Carrega databases do servidor após conectar (apenas nomes)
          await loadChildren({
            parentKey: key("server", server.id),
            fetcher: () => getPostgreDatabases(server.id),
            mapItem: (db: any) => ({
              key: key("database", server.id, db.name),
              name: db.name,
              data: { ...db, server_id: server.id },
            }),
            emptyMsg: "Nenhum database encontrado para este servidor.",
            successMsg: `Databases do servidor "${server.name}" carregadas!`,
          });

          return hasConnected;
        },
      });
    },
    [isConnecting, run, tree, loadChildren]
  );

  useEffect(() => {
    if (autoFetch) fetchServers();
  }, [autoFetch, fetchServers]);

  return {
    // estado
    servers,
    setServers,
    isLoading,
    isConnecting,
    error,
    // ações
    fetchServers,
    addServer,
    getServer,
    editServer,
    removeServer,
    connectToServer,
  } as const;
}
