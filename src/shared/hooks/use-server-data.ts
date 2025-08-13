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
import {
  getPostgreSchemas,
  getPostgreTables,
  getPostgreColumns,
  getPostgreIndexes,
  getPostgreTriggers,
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
 * Hook 1 — Responsável por:
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

/**
 * Hook 2 — Responsável por:
 *  - Carregar e salvar a **estrutura**: schemas, tabelas, colunas, índices, gatilhos
 *
 * Requer um `setServers` para operar sobre a mesma árvore do hook de conexão.
 * Assim, ambos podem ser usados de forma independente e combinados quando necessário.
 */

export interface IUseDbStructureParams {
  setServers: React.Dispatch<React.SetStateAction<Record<string, ITreeNode>>>;
}

export function useDbStructure(params: IUseDbStructureParams) {
  const { setServers } = params;

  const [isLoading, setIsLoading] = useState(false);
  const [isConnecting, setIsConnecting] = useState(false); // mantido para compatibilidade com createRunner
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

  const tree = useMemo(() => createTreeActions(setServers), [setServers]);
  const loadChildren = useMemo(
    () => createLoadChildren(run, tree),
    [run, tree]
  );

  const getDatabaseSchemas = useCallback(
    async (serverId: number, databaseName: string) => {
      const parent = key("database", serverId, databaseName);

      const schemas = await loadChildren({
        parentKey: parent,
        fetcher: () => getPostgreSchemas(serverId, databaseName),
        mapItem: (schema: any) => {
          const schemaKey = key("schema", serverId, databaseName, schema.name);
          const schemaTablesKey = key(
            "schema_table",
            serverId,
            databaseName,
            schema.name
          );
          // cria agrupador "Tabelas" como filho do schema
          tree.upsertNode(schemaTablesKey, "Tabelas", {
            ...schema,
            server_id: serverId,
          });
          // o próprio schema apontará para o agrupador após o load
          return {
            key: schemaKey,
            name: schema.name,
            data: { ...schema, server_id: serverId },
          };
        },
        emptyMsg: "Nenhum schema encontrado para este servidor.",
        successMsg: `Schemas do database "${databaseName}" carregados com sucesso!`,
      });

      // Vincula cada schema ao seu agrupador "Tabelas"
      setServers((prev) => {
        const next = { ...prev } as Record<string, ITreeNode>;
        schemas?.forEach((s: any) => {
          const schemaKey = key("schema", serverId, databaseName, s.name);
          const schemaTablesKey = key(
            "schema_table",
            serverId,
            databaseName,
            s.name
          );
          if (next[schemaKey])
            next[schemaKey] = {
              ...next[schemaKey],
              children: [schemaTablesKey],
            };
        });
        return next;
      });

      return schemas;
    },
    [loadChildren, tree, setServers]
  );

  const getSchemaTables = useCallback(
    async (serverId: number, schemaName: string, databaseName: string) => {
      const parent = key("schema_table", serverId, databaseName, schemaName);

      return loadChildren({
        parentKey: parent,
        fetcher: () => getPostgreTables(serverId, schemaName, databaseName),
        mapItem: (tbl: any) => {
          const tblKey = key(
            "table",
            serverId,
            databaseName,
            schemaName,
            tbl.name
          );
          const tblColumnsKey = key(
            "table_column",
            serverId,
            databaseName,
            schemaName,
            tbl.name
          );
          const tblIndexesKey = key(
            "table_index",
            serverId,
            databaseName,
            schemaName,
            tbl.name
          );
          const tblTriggersKey = key(
            "table_trigger",
            serverId,
            databaseName,
            schemaName,
            tbl.name
          );

          // cria nós do agrupamento e define children do nó da tabela
          tree.upsertNode(tblKey, tbl.name, { ...tbl, schema: schemaName });
          tree.upsertNode(tblColumnsKey, "Colunas", {
            ...tbl,
            schema: schemaName,
          });
          tree.upsertNode(tblIndexesKey, "Índices", {
            ...tbl,
            schema: schemaName,
          });
          tree.upsertNode(tblTriggersKey, "Triggers", {
            ...tbl,
            schema: schemaName,
          });
          tree.setChildren(tblKey, [
            tblColumnsKey,
            tblIndexesKey,
            tblTriggersKey,
          ]);

          return {
            key: tblKey,
            name: tbl.name,
            data: { ...tbl, schema: schemaName },
          };
        },
        emptyMsg: `Nenhuma tabela encontrada para o schema "${schemaName}".`,
        successMsg: `Tabelas do schema "${schemaName}" carregadas com sucesso!`,
      });
    },
    [loadChildren, tree]
  );

  const getTableColumns = useCallback(
    async (
      serverId: number,
      schemaName: string,
      tableName: string,
      databaseName: string
    ) => {
      const parent = key(
        "table_column",
        serverId,
        databaseName,
        schemaName,
        tableName
      );

      return loadChildren({
        parentKey: parent,
        fetcher: () =>
          getPostgreColumns(serverId, schemaName, tableName, databaseName),
        mapItem: (col: any) => ({
          key: key(
            "column",
            serverId,
            databaseName,
            schemaName,
            tableName,
            col.name
          ),
          name: col.name,
          data: col,
        }),
        emptyMsg: "Nenhuma coluna encontrada para esta tabela.",
        successMsg: `Colunas da tabela "${tableName}" carregadas com sucesso!`,
      });
    },
    [loadChildren]
  );

  const getTableIndexes = useCallback(
    async (
      serverId: number,
      schemaName: string,
      tableName: string,
      databaseName: string
    ) => {
      const parent = key(
        "table_index",
        serverId,
        databaseName,
        schemaName,
        tableName
      );

      return loadChildren({
        parentKey: parent,
        fetcher: () =>
          getPostgreIndexes(serverId, schemaName, tableName, databaseName),
        mapItem: (idx: any) => ({
          key: key(
            "index",
            serverId,
            databaseName,
            schemaName,
            tableName,
            idx.name
          ),
          name: idx.name,
          data: idx,
        }),
        emptyMsg: "Nenhum índice encontrado para esta tabela.",
        successMsg: `Índices da tabela "${tableName}" carregadas com sucesso!`,
      });
    },
    [loadChildren]
  );

  const getTableTriggers = useCallback(
    async (
      serverId: number,
      schemaName: string,
      tableName: string,
      databaseName: string
    ) => {
      const parent = key(
        "table_trigger",
        serverId,
        databaseName,
        schemaName,
        tableName
      );

      return loadChildren({
        parentKey: parent,
        fetcher: () =>
          getPostgreTriggers(serverId, schemaName, tableName, databaseName),
        mapItem: (trg: any) => ({
          key: key(
            "trigger",
            serverId,
            databaseName,
            schemaName,
            tableName,
            trg.name
          ),
          name: trg.name,
          data: trg,
        }),
        emptyMsg: "Nenhum gatilho encontrado para esta tabela.",
        successMsg: `Gatilhos da tabela "${tableName}" carregados com sucesso!`,
      });
    },
    [loadChildren]
  );

  return {
    isLoading,
    isConnecting,
    error,
    getDatabaseSchemas,
    getSchemaTables,
    getTableColumns,
    getTableIndexes,
    getTableTriggers,
  } as const;
}
