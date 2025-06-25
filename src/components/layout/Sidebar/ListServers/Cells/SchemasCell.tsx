import React from "react";
import { FolderTree } from "lucide-react";

import BaseCell from "./BaseCell";

import { IPostgreSchema } from "@/models/postgreDb";

interface BaseSchemasCellProps {
  schema: IPostgreSchema;
}

export default function BaseSchemasCell({ schema }: BaseSchemasCellProps) {
  return (
    <BaseCell
      icon={<FolderTree className="size-4" />}
      primaryText={schema.name}
    />
  );
}
