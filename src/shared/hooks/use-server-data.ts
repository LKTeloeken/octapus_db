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

export function useServersData() {
  const [servers, setServers] = useState<Record<string, ITreeNode>>({});
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

  const buildKey = (...parts: (string | number)[]) => parts.join("::");

  const updateServerInTree = (
    serverId: number,
    data: any,
    remove?: boolean
  ) => {
    setServers((prev) => {
      const updated = { ...prev };
      const serverKey = buildKey("server", serverId);

      if (remove && !updated[serverKey]) return updated;
      if (remove && updated[serverKey]) {
        delete updated[serverKey];
        return updated;
      }

      if (!updated[serverKey]) {
        updated[serverKey] = { name: data.name, children: [], data };
      }

      if (updated[serverKey]) {
        updated[serverKey].name = data.name || updated[serverKey].name;
        updated[serverKey].data = {
          ...(updated[serverKey].data || {}),
          ...data,
        };
      }

      return updated;
    });
  };

  const fetchServers = useCallback(async () => {
    initLoadingState();

    try {
      const serverList = await getAllServers();
      const tree: Record<string, ITreeNode> = {};

      serverList.forEach((srv) => {
        const srvKey = buildKey("server", srv.id);
        tree[srvKey] = { name: srv.name, children: [], data: srv };
      });

      setServers(tree);
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

      updateServerInTree(newSrv.id, newSrv);

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

  const editServer = useCallback(async (id: number, data: IServerPrimitive) => {
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

      updateServerInTree(id, updated);

      toast.success(`Servidor "${updated.name}" atualizado!`);
      return updated;
    } catch (err) {
      handleError(err);
    } finally {
      setIsLoading(false);
    }
  }, []);

  const removeServer = useCallback(async (id: number) => {
    initLoadingState();

    try {
      await deleteServer(id);

      updateServerInTree(id, {}, true);

      toast.success(`Servidor removido com sucesso!`);
    } catch (err) {
      handleError(err);
    } finally {
      setIsLoading(false);
    }
  }, []);

  const connectToServer = useCallback(async (server: IServer) => {
    console.log("Connecting to server:", server);

    if (server.isConnected) return true;
    if (isConnecting) return;

    initLoadingState();

    try {
      const hasConnected = await connectToPostgreServer(
        server.id,
        server.default_database
      );

      console.log("Connection result:", hasConnected);

      updateServerInTree(server.id, {
        isConnected: hasConnected,
        default_database: server.default_database,
      });

      toast.success(`Conectado ao servidor "${server.name}"!`);

      try {
        const serverDatabases = await getPostgreDatabases(server.id);

        if (!serverDatabases || !serverDatabases?.length) return hasConnected;

        setServers((prev) => {
          const updated = { ...prev };
          const srvKey = buildKey("server", server.id);
          const dbKeys: string[] = [];

          serverDatabases.forEach((db) => {
            const dbKey = buildKey("database", server.id, db.name);
            dbKeys.push(dbKey);
            updated[dbKey] = {
              name: db.name,
              children: [],
              data: { ...db, server_id: server.id },
            };
          });

          if (updated[srvKey]) {
            updated[srvKey].children = dbKeys;
          }

          return updated;
        });

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
    async (serverId: number, databaseName: string) => {
      initLoadingState();

      try {
        const schemas = await getPostgreSchemas(serverId, databaseName);

        if (!schemas || !schemas.length) {
          toast.error("Nenhum schema encontrado para este servidor.");
          return [];
        }

        setServers((prev) => {
          const updated = { ...prev };
          const dbKey = buildKey("database", serverId, databaseName);
          const schemaKeys: string[] = [];

          schemas.forEach((schema) => {
            const schemaKey = buildKey(
              "schema",
              serverId,
              databaseName,
              schema.name
            );
            const schemaTablesKey = buildKey(
              "schema_table",
              serverId,
              databaseName,
              schema.name
            );
            schemaKeys.push(schemaKey);
            updated[schemaKey] = {
              name: schema.name,
              children: [schemaTablesKey],
              data: { ...schema, server_id: serverId },
            };

            updated[schemaTablesKey] = {
              name: "Tabelas",
              children: [],
              data: { ...schema, server_id: serverId },
            };
          });

          if (updated[dbKey]) {
            updated[dbKey].children = schemaKeys;
          }

          return updated;
        });

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

  const getSchemaTables = useCallback(
    async (serverId: number, schemaName: string, databaseName: string) => {
      initLoadingState();

      try {
        const tables = await getPostgreTables(
          serverId,
          schemaName,
          databaseName
        );

        if (!tables || !tables.length) {
          toast.error("Nenhuma tabela encontrada para este schema.");
          return [];
        }

        setServers((prev) => {
          const updated = { ...prev };
          const schemaKey = buildKey(
            "schema_table",
            serverId,
            databaseName,
            schemaName
          );
          const tableKeys: string[] = [];

          tables.forEach((tbl) => {
            const tblKey = buildKey(
              "table",
              serverId,
              databaseName,
              schemaName,
              tbl.name
            );
            const tblColumnsKey = buildKey(
              "table_column",
              serverId,
              databaseName,
              schemaName,
              tbl.name
            );
            const tblIndexesKey = buildKey(
              "table_index",
              serverId,
              databaseName,
              schemaName,
              tbl.name
            );
            const tblTriggersKey = buildKey(
              "table_trigger",
              serverId,
              databaseName,
              schemaName,
              tbl.name
            );

            tableKeys.push(tblKey);
            updated[tblKey] = {
              name: tbl.name,
              children: [tblColumnsKey, tblIndexesKey, tblTriggersKey],
              data: { ...tbl, schema: schemaName },
            };

            updated[tblColumnsKey] = {
              name: "Colunas",
              children: [],
              data: { ...tbl, schema: schemaName },
            };
            updated[tblIndexesKey] = {
              name: "Índices",
              children: [],
              data: { ...tbl, schema: schemaName },
            };
            updated[tblTriggersKey] = {
              name: "Triggers",
              children: [],
              data: { ...tbl, schema: schemaName },
            };
          });

          if (updated[schemaKey]) updated[schemaKey].children = tableKeys;
          return updated;
        });

        toast.success(
          `Tabelas do schema "${schemaName}" carregadas com sucesso!`
        );

        return tables;
      } catch (err) {
        handleError(err);
      } finally {
        setIsLoading(false);
      }
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
      initLoadingState();

      try {
        const columns = await getPostgreColumns(
          serverId,
          schemaName,
          tableName,
          databaseName
        );

        if (!columns || !columns.length) {
          toast.error("Nenhuma coluna encontrada para esta tabela.");
          return [];
        }

        setServers((prev) => {
          const updated = { ...prev };
          const tableKey = buildKey(
            "table_column",
            serverId,
            databaseName,
            schemaName,
            tableName
          );
          const colKeys: string[] = [];

          columns.forEach((col) => {
            const colKey = buildKey(
              "column",
              serverId,
              databaseName,
              schemaName,
              tableName,
              col.name
            );
            colKeys.push(colKey);
            updated[colKey] = { name: col.name, data: col };
          });

          if (updated[tableKey]) updated[tableKey].children = colKeys;
          return updated;
        });

        toast.success(
          `Colunas da tabela "${tableName}" carregadas com sucesso!`
        );

        return columns;
      } catch (err) {
        handleError(err);
      } finally {
        setIsLoading(false);
      }
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
      initLoadingState();

      try {
        const indexes = await getPostgreIndexes(
          serverId,
          schemaName,
          tableName,
          databaseName
        );

        if (!indexes || !indexes.length) {
          toast.error("Nenhum índice encontrado para esta tabela.");
          return [];
        }

        setServers((prev) => {
          const updated = { ...prev };
          const tableKey = buildKey(
            "table_index",
            serverId,
            databaseName,
            schemaName,
            tableName
          );
          const idxKeys: string[] = [];

          indexes.forEach((idx) => {
            const idxKey = buildKey(
              "index",
              serverId,
              databaseName,
              schemaName,
              tableName,
              idx.name
            );
            idxKeys.push(idxKey);
            updated[idxKey] = { name: idx.name, data: idx };
          });

          if (updated[tableKey]) updated[tableKey].children = idxKeys;
          return updated;
        });

        toast.success(
          `Índices da tabela "${tableName}" carregados com sucesso!`
        );

        return indexes;
      } catch (err) {
        handleError(err);
      } finally {
        setIsLoading(false);
      }
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
      initLoadingState();

      try {
        const triggers = await getPostgreTriggers(
          serverId,
          schemaName,
          tableName,
          databaseName
        );

        if (!triggers || !triggers.length) {
          toast.error("Nenhum gatilho encontrado para esta tabela.");
          return [];
        }

        setServers((prev) => {
          const updated = { ...prev };
          const tableKey = buildKey(
            "table_index",
            serverId,
            databaseName,
            schemaName,
            tableName
          );
          const trgKeys: string[] = [];

          triggers.forEach((trg) => {
            const trgKey = buildKey(
              "trigger",
              serverId,
              databaseName,
              schemaName,
              tableName,
              trg.name
            );
            trgKeys.push(trgKey);
            updated[trgKey] = { name: trg.name, data: trg };
          });

          if (updated[tableKey]) updated[tableKey].children = trgKeys;
          return updated;
        });

        toast.success(
          `Gatilhos da tabela "${tableName}" carregados com sucesso!`
        );

        return triggers;
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
