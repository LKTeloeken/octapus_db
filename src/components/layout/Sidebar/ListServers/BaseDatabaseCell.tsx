import React from "react";
import {
  ListItem,
  ListItemText,
  ListItemButton,
  ListItemIcon,
} from "@/components/ui/list-item";
import { Database } from "lucide-react";

import BaseSchemasCell from "./BaseSchemasCell";

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
  return (
    <>
      <ListItem key={database.name} disablePadding>
        <ListItemButton
          onClick={() =>
            getDatabaseSchemas?.(database.server_id, database.name)
          }
        >
          <ListItemIcon className="flex items-center justify-center">
            <Database className="size-4" />
          </ListItemIcon>
          <ListItemText primary={database.name} />
        </ListItemButton>
      </ListItem>

      {database.schemas && database.schemas.length > 0 && (
        <div className="pl-4">
          {database.schemas.map((schema) => (
            <BaseSchemasCell key={schema.name} schema={schema} />
          ))}
        </div>
      )}
    </>
  );
}
