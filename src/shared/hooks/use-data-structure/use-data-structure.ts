import { useCallback, useState } from "react";
import {
  getDatabases,
  getSchemas,
  getTables,
  getColumns,
} from "@/api/postgres/methods";
import { formatTreeNode } from "@/lib/format-tree-node";
import toast from "react-hot-toast";
import type { TreeState, TreeNode } from "./use-data-structure.types";
import type { TreeNodeType } from "@/shared/models/database.types";

export const useDataStructure = () => {
  const [state, setState] = useState<TreeState>({
    nodes: new Map(),
    childrenMap: new Map(),
  });

  const addChildrenToState = useCallback((childrens: TreeNode[]) => {
    setState((prev) => {
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

  const removeChildrenFromState = useCallback((parentId: string) => {
    setState((prev) => {
      const newNodes = new Map(prev.nodes);
      const newChildrenMap = new Map(prev.childrenMap);

      const childrenIds = newChildrenMap.get(parentId) || [];
      childrenIds.forEach((childId) => newNodes.delete(childId));
      newChildrenMap.delete(parentId);

      return { nodes: newNodes, childrenMap: newChildrenMap };
    });
  }, []);

  const getChildren = useCallback(
    async (nodeType: TreeNodeType, nodeId: string): Promise<TreeNode[]> => {
      const node = state.nodes.get(nodeId);
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
            nodeId
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
            nodeId
          )
        );
      }

      if (nodeType === "schema") {
        const databaseName = state.nodes.get(node.parentId!)?.name!;
        const schemaName = node.name;
        const tables = await getTables(serverId, databaseName, schemaName);

        return tables.map((table) =>
          formatTreeNode(
            `table-${table.name}-${schemaName}-${databaseName}-${serverId}`,
            "table",
            serverId,
            table.name,
            nodeId
          )
        );
      }

      if (nodeType === "table") {
        const databaseName = state.nodes.get(
          state.nodes.get(node.parentId!)?.parentId!
        )?.name!;
        const schemaName = state.nodes.get(node.parentId!)?.name!;
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
            nodeId
          )
        );
      }

      return [];
    },
    [state.nodes]
  );

  // Lazy load children when node expands
  const loadChildren = useCallback(
    async (nodeId: string) => {
      const node = state.nodes.get(nodeId);
      if (!node || !node.hasChildren) return;

      setState((prev) => {
        const newNodes = new Map(prev.nodes);
        newNodes.set(nodeId, { ...node, isLoading: true });
        return { ...prev, nodes: newNodes };
      });

      try {
        const childrens = await getChildren(node.type, nodeId);

        if (childrens.length === 0) return;

        setState((prev) => {
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
        setState((prev) => {
          const newNodes = new Map(prev.nodes);
          newNodes.set(nodeId, { ...node, isLoading: false });
          return { ...prev, nodes: newNodes };
        });
      }
    },
    [state.nodes, getChildren]
  );

  const toggleNode = useCallback(
    (nodeId: string) => {
      const node = state.nodes.get(nodeId);
      if (!node) return;

      const shouldExpand = !node.isExpanded;

      setState((prev) => ({
        ...prev,
        nodes: new Map(prev.nodes).set(nodeId, {
          ...node,
          isExpanded: shouldExpand,
        }),
      }));

      if (shouldExpand && !state.childrenMap.has(nodeId)) {
        loadChildren(nodeId);
      }
    },
    [state, loadChildren]
  );

  return {
    state,
    toggleNode,
    loadChildren,
    addChildrenToState,
    removeChildrenFromState,
  };
};
