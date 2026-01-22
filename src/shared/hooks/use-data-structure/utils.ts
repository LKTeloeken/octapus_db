import { formatTreeNode } from "@/lib/format-tree-node";
import type {
  DatabaseStructure,
  TreeNode,
} from "@/shared/models/database.types";
import { TreeNodeType } from "@/shared/models/database.types";

export const convertDatabaseStructureToNodes = (
  serverId: number,
  databaseName: string,
  structure: DatabaseStructure,
) => {
  const nodes: TreeNode[] = [];

  structure.schemas.forEach((schema) => {
    const schemaNode: TreeNode = formatTreeNode(
      `schema-${schema.name}-${databaseName}-${serverId}`,
      TreeNodeType.Schema,
      serverId,
      schema.name,
      `database-${databaseName}-${serverId}`,
      {
        type: TreeNodeType.Schema,
        serverId,
        databaseName,
      },
    );
    nodes.push(schemaNode);

    schema.tables.forEach((table) => {
      const tableNode: TreeNode = formatTreeNode(
        `table-${table.name}-${schema.name}-${databaseName}-${serverId}`,
        TreeNodeType.Table,
        serverId,
        table.name,
        schemaNode.id,
        {
          type: TreeNodeType.Table,
          serverId,
          databaseName,
        },
      );
      nodes.push(tableNode);

      if (!table.columns) return;

      table.columns.forEach((column) => {
        const columnNode: TreeNode = formatTreeNode(
          `column-${column.name}-${table.name}-${schema.name}-${databaseName}-${serverId}`,
          TreeNodeType.Column,
          serverId,
          column.name,
          tableNode.id,
          {
            type: TreeNodeType.Column,
            serverId,
            databaseName,
            dataType: column.data_type,
            isNullable: column.is_nullable,
          },
        );
        nodes.push(columnNode);
      });
    });
  });

  return nodes;
};
