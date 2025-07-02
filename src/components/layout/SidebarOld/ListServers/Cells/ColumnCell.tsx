import React from "react";
import { Columns } from "lucide-react";

import BaseCell from "./BaseCell";

import { IPostgreColumn } from "@/shared/models/postgreDb";

interface BaseColumnCellProps {
  column: IPostgreColumn;
  serverId: number;
  schemaName: string;
  tableName: string;
  databaseName?: string;
}

export default function BaseColumnCell({
  column,
  serverId,
  schemaName,
  tableName,
  databaseName,
}: BaseColumnCellProps) {
  const handleClick = () => {
    // Columns are leaf nodes, no further expansion needed
    console.log(`Column clicked: ${column.name} in ${schemaName}.${tableName}`);
  };

  // Format the column display with type and nullable info
  const getColumnDisplayInfo = () => {
    const typeInfo = `${column.data_type}${
      !column.is_nullable ? " NOT NULL" : ""
    }`;
    return {
      primary: column.name,
      secondary: typeInfo,
    };
  };

  const displayInfo = getColumnDisplayInfo();

  return (
    <BaseCell
      icon={<Columns className="size-4" />}
      primaryText={displayInfo.primary}
      secondaryText={displayInfo.secondary}
      onClick={handleClick}
    />
  );
}
