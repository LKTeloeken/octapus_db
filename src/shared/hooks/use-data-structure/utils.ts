import { formatTreeNode } from '@/lib/format-tree-node';
import type {
  DatabaseStructure,
  TreeNode,
} from '@/shared/models/database.types';
import { TreeNodeType } from '@/shared/models/database.types';

export const convertDatabaseStructureToNodes = (
  serverId: number,
  databaseName: string,
  structure: DatabaseStructure,
): TreeNode[] => {
  const dbSuffix = `${databaseName}-${serverId}`;
  const parentId = `database-${dbSuffix}`;
  const nodes: TreeNode[] = [];

  for (const schema of structure.schemas) {
    const schemaSuffix = `${schema.name}-${dbSuffix}`;
    const schemaId = `schema-${schemaSuffix}`;

    nodes.push(
      formatTreeNode(
        schemaId,
        TreeNodeType.Schema,
        serverId,
        schema.name,
        parentId,
        {
          type: TreeNodeType.Schema,
          serverId,
          databaseName,
        },
      ),
    );

    for (const table of schema.tables) {
      const tableSuffix = `${table.name}-${schemaSuffix}`;
      const tableId = `table-${tableSuffix}`;

      nodes.push(
        formatTreeNode(
          tableId,
          TreeNodeType.Table,
          serverId,
          table.name,
          schemaId,
          {
            type: TreeNodeType.Table,
            serverId,
            databaseName,
          },
        ),
      );

      if (table.columns) {
        for (const column of table.columns) {
          nodes.push(
            formatTreeNode(
              `column-${column.name}-${tableSuffix}`,
              TreeNodeType.Column,
              serverId,
              column.name,
              tableId,
              {
                type: TreeNodeType.Column,
                serverId,
                databaseName,
                dataType: column.data_type,
                isNullable: column.is_nullable,
              },
            ),
          );
        }
      }
    }
  }

  return nodes;
};
