import React from "react";
import BaseCell from "@/components/common/server/base-cell";
import {
  Columns,
  Database,
  FolderTree,
  Server,
  Table,
  Zap,
  Folder,
  Hash,
  Key,
  KeyRound,
  Bolt,
} from "lucide-react";

import type { renderItemFunction } from "@/components/common/recursive-list";
import type * as HookTypes from "@/shared/models/server-functions";
import { IServer } from "@/shared/models/server";
import { IPostgreColumn } from "@/shared/models/postgreDb";

export const renderItems =
  (
    isLoading: boolean,
    connectToServer: HookTypes.ConnectToServerFunction,
    getDatabaseSchemas: HookTypes.GetDatabaseSchemasFunction,
    getSchemaTables: HookTypes.GetSchemaTablesFunction,
    getTableColumns: HookTypes.GetTableColumnsFunction,
    getTableIndexes: HookTypes.GetTableIndexesFunction,
    getTableTriggers: HookTypes.GetTableTriggersFunction
  ): renderItemFunction =>
  (item, onClick, _, isExpanded, __) => {
    const { type } = item;
    const nodesWithChildren = [
      "server",
      "database",
      "schema",
      "schema_table",
      "table",
      "table_column",
      "table_index",
      "table_trigger",
    ];
    const hasChildren = nodesWithChildren.includes(type);

    const getIconForType = (type: string) => {
      switch (type) {
        case "server":
          return <Server className="size-4" />;
        case "database":
          return <Database className="size-4" />;
        case "schema":
          return <Folder className="size-4" />;
        case "schema_table":
          return <FolderTree className="size-4" />;
        case "table":
          return <Table className="size-4" />;
        case "table_column":
          return <Columns className="size-4" />;
        case "table_index":
          return <KeyRound className="size-4" />;
        case "table_trigger":
          return <Zap className="size-4" />;
        case "column":
          return <Hash className="size-4" />;
        case "index":
          return <Key className="size-4" />;
        case "trigger":
          return <Bolt className="size-4" />;
        default:
          return null;
      }
    };

    const clickServer = async () => {
      await connectToServer(item.data as IServer);
    };

    const clickDatabase = async () => {
      const [, serverId, databaseName] = item.itemKey.split("::");

      await getDatabaseSchemas(Number(serverId), databaseName);
    };

    const clickSchema = async () => {
      const [, serverId, databaseName, schemaName] = item.itemKey.split("::");

      await getSchemaTables(Number(serverId), schemaName, databaseName);
    };

    const clickTableColumn = async () => {
      const [, serverId, databaseName, schemaName, tableName] =
        item.itemKey.split("::");

      await getTableColumns(
        Number(serverId),
        schemaName,
        tableName,
        databaseName
      );
    };

    const clickTableIndex = async () => {
      const [, serverId, databaseName, schemaName, tableName] =
        item.itemKey.split("::");

      await getTableIndexes(
        Number(serverId),
        schemaName,
        tableName,
        databaseName
      );
    };

    const clickTableTrigger = async () => {
      const [, serverId, databaseName, schemaName, tableName] =
        item.itemKey.split("::");

      await getTableTriggers(
        Number(serverId),
        schemaName,
        tableName,
        databaseName
      );
    };

    const getSecondaryText = (): string => {
      if (type === "column") {
        return `${(item?.data as IPostgreColumn).data_type}${
          !(item?.data as IPostgreColumn).is_nullable ? " NOT NULL" : ""
        }`;
      }

      return "";
    };

    const functionsByType: Record<string, (...args: any[]) => void> = {
      server: clickServer,
      database: clickDatabase,
      schema_table: clickSchema,
      table_column: clickTableColumn,
      table_index: clickTableIndex,
      table_trigger: clickTableTrigger,
    };

    const handleClick = async () => {
      const func = functionsByType[type];

      onClick?.();
      if (func && !isExpanded) await func();
    };

    return (
      <BaseCell
        icon={getIconForType(type)}
        primaryText={item.name}
        secondaryText={getSecondaryText()}
        onClick={handleClick}
        isExpanded={isExpanded}
        hasChildren={hasChildren}
        disabled={isLoading}
      />
    );
  };
