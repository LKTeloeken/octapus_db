import React from "react";
import BaseCell from "@/components/common/server/base-cell";
import { Columns, Database, FolderTree, Server, Table } from "lucide-react";

import type { renderItemFunction } from "@/components/common/recursive-list";
import type * as HookTypes from "@/shared/models/server-functions";
import { IServer } from "@/shared/models/server";
import { IPostgreDatabase, IPostgreSchema } from "@/shared/models/postgreDb";

export const renderItems =
  (
    connectToServer: HookTypes.ConnectToServerFunction,
    getDatabaseSchemas: HookTypes.GetDatabaseSchemasFunction,
    getSchemaTables: HookTypes.GetSchemaTablesFunction,
    getSchemaColumns: HookTypes.GetSchemaColumnsFunction
  ): renderItemFunction =>
  (item, onClick, level, isExpanded, hasChildren) => {
    const { type } = item;

    const getIconForType = (type: string) => {
      switch (type) {
        case "server":
          return <Server className="size-4" />;
        case "database":
          return <Database className="size-4" />;
        case "schema":
          return <FolderTree className="size-4" />;
        case "table":
          return <Table className="size-4" />;
        case "column":
          return <Columns className="size-4" />;
        default:
          return null;
      }
    };

    const functionsByType: Record<string, (...args: any[]) => void> = {
      server: connectToServer,
      database: getDatabaseSchemas,
      schema: getSchemaTables,
      table: getSchemaColumns,
    };

    const handleClick = () => {};

    return (
      <BaseCell
        icon={getIconForType(type)}
        primaryText={item.name}
        onClick={onClick}
        isExpanded={isExpanded}
        hasChildren={hasChildren}
      />
    );
  };
