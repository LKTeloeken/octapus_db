import React from "react";
import {
  ListItem,
  ListItemText,
  ListItemButton,
  ListItemIcon,
} from "@/components/ui/list-item";
import { FolderTree } from "lucide-react";

import { IPostgreSchema } from "@/models/postgreDb";

interface BaseSchemasCellProps {
  schema: IPostgreSchema;
}

export default function BaseSchemasCell({ schema }: BaseSchemasCellProps) {
  return (
    <>
      <ListItem key={schema.name} disablePadding>
        <ListItemButton>
          <ListItemIcon className="flex items-center justify-center">
            <FolderTree className="size-4" />
          </ListItemIcon>
          <ListItemText primary={schema.name} />
        </ListItemButton>
      </ListItem>
    </>
  );
}
