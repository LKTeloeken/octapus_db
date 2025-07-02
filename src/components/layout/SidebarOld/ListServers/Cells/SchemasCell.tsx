import React from "react";
import { FolderTree } from "lucide-react";

import BaseCell from "./BaseCell";
import BaseTableCell from "./TableCell";

import { IPostgreSchema } from "@/shared/models/postgreDb";

interface BaseSchemasCellProps {
  schema: IPostgreSchema;
  serverId: number;
  databaseName?: string;
  getSchemaTables?: (
    serverId: number,
    schemaName: string,
    databaseName?: string
  ) => Promise<any>;
  getSchemaColumns?: (
    serverId: number,
    schemaName: string,
    tableName: string,
    databaseName?: string
  ) => Promise<any>;
}

export default function BaseSchemasCell({
  schema,
  serverId,
  databaseName,
  getSchemaTables,
  getSchemaColumns,
}: BaseSchemasCellProps) {
  const handleClick = () => {
    getSchemaTables?.(serverId, schema.name, databaseName);
  };

  return (
    <BaseCell
      icon={<FolderTree className="size-4" />}
      primaryText={schema.name}
      onClick={handleClick}
    >
      {schema.tables?.length > 0 &&
        schema.tables.map((table) => (
          <BaseTableCell
            key={table.name}
            table={table}
            serverId={serverId}
            schemaName={schema.name}
            databaseName={databaseName}
            getSchemaColumns={getSchemaColumns}
          />
        ))}
    </BaseCell>
  );
}
