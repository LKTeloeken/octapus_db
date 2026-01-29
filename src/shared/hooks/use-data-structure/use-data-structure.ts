import { useCallback, useState } from "react";
import {
  getDatabases,
  getSchemasWithTables,
  getColumns,
} from "@/api/database/database-methods";
import { formatTreeNode } from "@/lib/format-tree-node";
import { convertDatabaseStructureToNodes } from "./utils";
import toast from "react-hot-toast";
import type {
  RemoveNode,
  TreeState,
  AddNodes,
  HandleFetchStructure,
} from "./use-data-structure.types";
import { useStore } from "@/stores";
import { type TreeNode, TreeNodeType } from "@/shared/models/database.types";

export const useDataStructure = () => {
  const { fetchStructure, getStructure } = useStore();
  const [nodes, setNodes] = useState<TreeState>({
    nodes: new Map(),
    childrenMap: new Map(),
  });

  const handleSetNode = useCallback(
    (nodeId: string, nodeData: Partial<TreeNode>) => {
      setNodes((prev) => {
        const newNodes = new Map(prev.nodes);
        const existingNode = newNodes.get(nodeId);

        if (existingNode) {
          newNodes.set(nodeId, { ...existingNode, ...nodeData });
        }

        return { ...prev, nodes: newNodes };
      });
    },
    [],
  );

  const handleSetNodes = useCallback((nodesToUpdate: TreeNode[]) => {
    setNodes((prev) => {
      const newNodes = new Map(prev.nodes);
      const newChildrenMap = new Map(prev.childrenMap);

      const childrentsToUpdateMap: Map<string, string[]> = new Map();

      nodesToUpdate.forEach((node) => {
        const existingNode = newNodes.get(node.id) || node;

        newNodes.set(node.id, {
          ...node,
          isConnected: existingNode.isConnected,
          isExpanded: existingNode.isExpanded,
          hasLoadedChildren: existingNode.hasLoadedChildren,
        });

        if (node.parentId) {
          if (!childrentsToUpdateMap.has(node.parentId)) {
            childrentsToUpdateMap.set(node.parentId, []);
          }

          childrentsToUpdateMap.get(node.parentId)?.push(node.id);
        }
      });

      childrentsToUpdateMap.forEach((childrenIds, parentId) => {
        newChildrenMap.set(parentId, childrenIds);
      });

      return { nodes: newNodes, childrenMap: newChildrenMap };
    });
  }, []);

  const addNodes: AddNodes = useCallback(
    (nodesToAdd: TreeNode[]) => handleSetNodes(nodesToAdd),
    [handleSetNodes],
  );

  const removeNode: RemoveNode = useCallback((nodeId: string) => {
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

  const handleFetchStructure: HandleFetchStructure = useCallback(
    async (serverId: number, databaseName: string) => {
      const structureNodes = await getSchemasWithTables(serverId, databaseName);

      const formattedNodes = convertDatabaseStructureToNodes(
        serverId,
        databaseName,
        {
          schemas: structureNodes,
        },
      );

      handleSetNodes(formattedNodes);
    },
    [fetchStructure],
  );

  const handleLoadDatabaseStructure = useCallback(
    async (node: TreeNode) => {
      console.log("node", node);

      const serverId = node.metadata.serverId;

      if (node.type === TreeNodeType.Server) {
        const databases = await getDatabases(serverId);

        const databasesNodes = databases.map((db) =>
          formatTreeNode(
            `database-${db.name}-${serverId}`,
            TreeNodeType.Database,
            serverId,
            db.name,
            node.id,
            {
              type: TreeNodeType.Database,
              serverId,
              databaseName: db.name,
            },
          ),
        );

        return databasesNodes;
      }

      if (node.type === TreeNodeType.Database) {
        const databaseName = node.name;

        const structureNodes = await getSchemasWithTables(
          serverId,
          databaseName,
        );

        const nodes = convertDatabaseStructureToNodes(serverId, databaseName, {
          schemas: structureNodes,
        });

        return nodes;
      }

      if (node.type === TreeNodeType.Table) {
        const [, tableName, schemaName, databaseName, serverId] =
          node.id.split("-");

        console.log("infos", tableName, schemaName, databaseName, serverId);

        const columns = await getColumns(
          Number(serverId),
          databaseName,
          schemaName,
          tableName,
        );

        const columnNodes = columns.map((column) =>
          formatTreeNode(
            `column-${column.name}-${tableName}-${schemaName}-${databaseName}-${serverId}`,
            TreeNodeType.Column,
            Number(serverId),
            column.name,
            node.id,
            {
              type: TreeNodeType.Column,
              serverId: Number(serverId),
              databaseName,
            },
          ),
        );

        return columnNodes;
      }

      return [];
    },
    [fetchStructure],
  );

  const onClickNode = useCallback(
    async (nodeId: string) => {
      const node = nodes.nodes.get(nodeId);

      if (!node) return;

      if (
        node.type === TreeNodeType.Database ||
        node.type === TreeNodeType.Server ||
        node.type === TreeNodeType.Table
      ) {
        try {
          if (node.hasLoadedChildren) {
            handleSetNode(nodeId, { isExpanded: !node.isExpanded });
            return;
          }

          handleSetNode(nodeId, { isLoading: true });

          const childrenNodes = await handleLoadDatabaseStructure(node);

          handleSetNodes(childrenNodes);

          handleSetNode(nodeId, {
            isLoading: false,
            isExpanded: !node.isExpanded,
            hasLoadedChildren: true,
            isConnected: true,
          });

          return;
        } catch (error) {
          console.log("error", error);

          toast.error("Failed to load data structure.");
          handleSetNode(nodeId, { isLoading: false });
          return;
        }
      }

      handleSetNode(nodeId, { isExpanded: !node.isExpanded });
    },
    [nodes.nodes],
  );

  return {
    nodes,
    onClickNode,
    addNodes,
    removeNode,
    handleFetchStructure,
    getStructure,
  };
};
