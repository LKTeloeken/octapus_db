import React from "react";
import { Table } from "lucide-react";

import BaseCell from "./BaseCell";
import BaseColumnCell from "./ColumnCell";

import { IPostgreTable } from "@/shared/models/postgreDb";

interface BaseTableCellProps {
  table: IPostgreTable;
  serverId: number;
  schemaName: string;
  databaseName?: string;
  getSchemaColumns?: (
    serverId: number,
    schemaName: string,
    tableName: string,
    databaseName?: string
  ) => Promise<any>;
}

export default function BaseTableCell({
  table,
  serverId,
  schemaName,
  databaseName,
  getSchemaColumns,
}: BaseTableCellProps) {
  const handleClick = () => {
    getSchemaColumns?.(serverId, schemaName, table.name, databaseName);
  };

  return (
    <BaseCell
      icon={<Table className="size-4" />}
      primaryText={table.name}
      onClick={handleClick}
    >
      {table.columns?.length > 0 &&
        table.columns.map((column) => (
          <BaseColumnCell
            key={column.name}
            column={column}
            serverId={serverId}
            schemaName={schemaName}
            tableName={table.name}
            databaseName={databaseName}
          />
        ))}
    </BaseCell>
  );
}
