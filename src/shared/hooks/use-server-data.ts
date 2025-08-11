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
  getPostgreTables,
  getPostgreColumns,
  getPostgreIndexes,
  getPostgreTriggers,
} from "@/api/postgreMethods";
import { IServer, IServerPrimitive } from "@/shared/models/server";
import { ITreeNode } from "@/shared/models/tree";

import { toast } from "react-hot-toast";

import { createRunner } from "@/shared/utils/asyncRunner";
import {
  NodeKey,
  key,
  createTreeActions,
  createLoadChildren,
} from "@/shared/utils/serverTree";

export function useServersData() {
  const [servers, setServers] = useState<Record<string, ITreeNode>>({});
  const [isLoading, setIsLoading] = useState(false);
  const [isConnecting, setIsConnecting] = useState(false);
  const [error, setError] = useState<string | null>(null);

  const run = createRunner({
    setIsLoading,
    setIsConnecting,
    setError,
  });

  const tree = createTreeActions(setServers);
  const loadChildren = createLoadChildren(run, tree);

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
  }, []);

  const addServer = useCallback(async (data: IServerPrimitive) => {
    return run({
      task: async () => {
        const newSrv = await createServer(data);
        tree.upsertNode(key("server", newSrv.id), newSrv.name, newSrv);
        toast.success(`Servidor "${newSrv.name}" adicionado!`);
        return newSrv;
      },
    });
  }, []);

  const getServer = useCallback(async (id: number) => {
    return run({
      task: () => getServerById(id),
    });
  }, []);

  const editServer = useCallback(async (id: number, data: IServerPrimitive) => {
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
  }, []);

  const removeServer = useCallback(async (id: number) => {
    return run({
      task: async () => {
        await deleteServer(id);
        tree.removeSubtree(key("server", id));
        toast.success("Servidor removido com sucesso!");
        return true;
      },
    });
  }, []);

  // =====================
  // Conexão e carregamentos
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

          // Carrega databases do servidor após conectar
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
    [isConnecting]
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
    []
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
    []
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
    []
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
        successMsg: `Índices da tabela "${tableName}" carregados com sucesso!`,
      });
    },
    []
  );

  const getTableTriggers = useCallback(
    async (
      serverId: number,
      schemaName: string,
      tableName: string,
      databaseName: string
    ) => {
      const parent = key(
        "table_trigger", // corrigido: antes usava table_index
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
    []
  );

  useEffect(() => {
    fetchServers();
  }, [fetchServers]);

  return {
    servers,
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
    getSchemaTables,
    getTableColumns,
    getTableIndexes,
    getTableTriggers,
  };
}
