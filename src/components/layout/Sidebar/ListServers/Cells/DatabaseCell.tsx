import React from "react";
import { Database } from "lucide-react";

import BaseCell from "./BaseCell";
import BaseSchemasCell from "./SchemasCell";

import { IPostgreDatabase } from "@/models/postgreDb";

interface BaseDatabaseCellProps {
  database: IPostgreDatabase;
  getDatabaseSchemas?: (
    serverId: number,
    databaseName?: string
  ) => Promise<any>;
}

export default function BaseDatabaseCell({
  database,
  getDatabaseSchemas,
}: BaseDatabaseCellProps) {
  const handleClick = () => {
    getDatabaseSchemas?.(database.server_id, database.name);
  };

  return (
    <>
      <BaseCell
        icon={<Database className="size-4" />}
        primaryText={database.name}
        onClick={handleClick}
      >
        {database.schemas?.length > 0 &&
          database.schemas!.map((schema) => (
            <BaseSchemasCell key={schema.name} schema={schema} />
          ))}
      </BaseCell>
    </>
  );
}
