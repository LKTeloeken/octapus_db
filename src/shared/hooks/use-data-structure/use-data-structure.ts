import { useCallback, useState } from "react";
import {
  getDatabases,
  getSchemas,
  getTables,
  getColumns,
} from "@/api/postgres/methods";
import { formatTreeNode } from "@/lib/format-tree-node";
import toast from "react-hot-toast";
import type { TreeState } from "./use-data-structure.types";
import type { TreeNode, TreeNodeType } from "@/shared/models/database.types";

export const useDataStructure = () => {
  const [nodes, setNodes] = useState<TreeState>({
    nodes: new Map(),
    childrenMap: new Map(),
  });

  const addChildrenToState = useCallback((childrens: TreeNode[]) => {
    setNodes((prev) => {
      const newNodes = new Map(prev.nodes);
      const newChildrenMap = new Map(prev.childrenMap);

      childrens.forEach((child) => newNodes.set(child.id, child));
      if (childrens.length > 0) {
        const parentId = childrens[0].parentId;
        if (parentId) {
          newChildrenMap.set(
            parentId,
            childrens.map((c) => c.id)
          );
        }
      }

      return { nodes: newNodes, childrenMap: newChildrenMap };
    });
  }, []);

  const removeNode = useCallback((nodeId: string) => {
    setNodes((prev) => {
      const newNodes = new Map(prev.nodes);
      const newChildrenMap = new Map(prev.childrenMap);

      const deleteNodeRecursively = (id: string) => {
        const children = newChildrenMap.get(id);
        if (children) {
          children.forEach(deleteNodeRecursively);
          newChildrenMap.delete(id);
        }
        newNodes.delete(id);
      };

      deleteNodeRecursively(nodeId);

      return { nodes: newNodes, childrenMap: newChildrenMap };
    });
  }, []);

  const getChildren = useCallback(
    async (nodeType: TreeNodeType, nodeId: string): Promise<TreeNode[]> => {
      const node = nodes.nodes.get(nodeId);
      if (!node) return [];

      const serverId = node.metadata?.serverId;

      if (nodeType === "server") {
        const databases = await getDatabases(serverId);

        return databases.map((db) =>
          formatTreeNode(
            `database-${db.name}-${serverId}`,
            "database",
            serverId,
            db.name,
            nodeId,
            {
              type: "database",
              serverId,
            }
          )
        );
      }

      if (nodeType === "database") {
        const databaseName = node.name;
        const schemas = await getSchemas(serverId, databaseName);

        return schemas.map((schema) =>
          formatTreeNode(
            `schema-${schema.name}-${databaseName}-${serverId}`,
            "schema",
            serverId,
            schema.name,
            nodeId,
            {
              type: "schema",
              serverId,
            }
          )
        );
      }

      if (nodeType === "schema") {
        const databaseName = nodes.nodes.get(node.parentId!)?.name!;
        const schemaName = node.name;
        const tables = await getTables(serverId, databaseName, schemaName);

        return tables.map((table) =>
          formatTreeNode(
            `table-${table.name}-${schemaName}-${databaseName}-${serverId}`,
            "table",
            serverId,
            table.name,
            nodeId,
            {
              type: "table",
              serverId,
            }
          )
        );
      }

      if (nodeType === "table") {
        const databaseName = nodes.nodes.get(
          nodes.nodes.get(node.parentId!)?.parentId!
        )?.name!;
        const schemaName = nodes.nodes.get(node.parentId!)?.name!;
        const tableName = node.name;
        const columns = await getColumns(
          serverId,
          databaseName,
          schemaName,
          tableName
        );

        return columns.map((column) =>
          formatTreeNode(
            `column-${column.name}-${tableName}-${schemaName}-${databaseName}-${serverId}`,
            "column",
            serverId,
            column.name,
            nodeId,
            {
              type: "column",
              serverId,
              dataType: column.data_type,
              isNullable: column.is_nullable,
              columnDefault: column.column_default,
            }
          )
        );
      }

      return [];
    },
    [nodes.nodes]
  );

  // Lazy load children when node expands
  const loadChildren = useCallback(
    async (nodeId: string) => {
      const node = nodes.nodes.get(nodeId);
      if (!node || !node.hasChildren) return;

      setNodes((prev) => {
        const newNodes = new Map(prev.nodes);
        newNodes.set(nodeId, { ...node, isLoading: true });
        return { ...prev, nodes: newNodes };
      });

      try {
        const childrens = await getChildren(node.type, nodeId);

        if (childrens.length === 0) return;

        setNodes((prev) => {
          const newNodes = new Map(prev.nodes);
          const newChildrenMap = new Map(prev.childrenMap);

          childrens.forEach((child) => newNodes.set(child.id, child));
          newChildrenMap.set(
            nodeId,
            childrens.map((c) => c.id)
          );

          newNodes.set(nodeId, { ...node, isLoading: false });

          return { nodes: newNodes, childrenMap: newChildrenMap };
        });
      } catch (error) {
        toast.error("Failed to load children.");
      } finally {
        setNodes((prev) => {
          const newNodes = new Map(prev.nodes);
          newNodes.set(nodeId, { ...node, isLoading: false });
          return { ...prev, nodes: newNodes };
        });
      }
    },
    [nodes.nodes, getChildren]
  );

  const toggleNode = useCallback(
    async (nodeId: string) => {
      const node = nodes.nodes.get(nodeId);
      if (!node) return;

      const shouldExpand = !node.isExpanded;

      if (!shouldExpand) {
        setNodes((prev) => {
          const newNodes = new Map(prev.nodes);
          newNodes.set(nodeId, {
            ...node,
            isExpanded: false,
          });
          return { ...prev, nodes: newNodes };
        });
        return;
      }

      if (!nodes.childrenMap.has(nodeId)) {
        // Carrega filhos
        await loadChildren(nodeId);

        // Expande após carregar
        setNodes((prev) => {
          const newNodes = new Map(prev.nodes);
          newNodes.set(nodeId, {
            ...node,
            isExpanded: true,
          });
          return { ...prev, nodes: newNodes };
        });
      } else {
        setNodes((prev) => {
          const newNodes = new Map(prev.nodes);
          newNodes.set(nodeId, { ...node, isExpanded: true });
          return { ...prev, nodes: newNodes };
        });
      }
    },
    [nodes, loadChildren]
  );

  return {
    nodes,
    toggleNode,
    loadChildren,
    addChildrenToState,
    removeNode,
  };
};
