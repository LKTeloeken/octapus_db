import { Server, Database, Folder, Table, Hash } from "lucide-react";
import type { TreeNodeType } from "@/shared/models/database.types";

export const useTreeNode = () => {
  const getNodeIcon = (type: TreeNodeType) => {
    switch (type) {
      case "server":
        return <Server className="size-4" />;
      case "database":
        return <Database className="size-4" />;
      case "schema":
        return <Folder className="size-4" />;
      case "table":
        return <Table className="size-4" />;
      case "column":
        return <Hash className="size-4" />;
      default:
        return null;
    }
  };

  return { getNodeIcon };
};
