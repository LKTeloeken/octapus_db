import { getDatabases } from '@/api/database/database-methods';
import { formatTreeNode } from '@/lib/format-tree-node';
import { type TreeNode, TreeNodeType } from '@/shared/models/database.types';
import { useStore } from '@/stores';
import { useCallback, useEffect, useRef, useState } from 'react';
import toast from 'react-hot-toast';
import type {
  AddNodes,
  HandleFetchStructure,
  RemoveNode,
  TreeState,
} from './use-data-structure.types';
import { convertDatabaseStructureToNodes } from './utils';

export const useDataStructure = () => {
  const { getStructure, fetchStructure, fetchColumns } = useStore();
  const [nodes, setNodes] = useState<TreeState>({
    nodes: new Map(),
    childrenMap: new Map(),
  });

  // Ref to access nodes in a stable way inside callbacks
  const nodesRef = useRef(nodes);
  useEffect(() => {
    nodesRef.current = nodes;
  }, [nodes]);

  const handleSetNode = useCallback(
    (nodeId: string, nodeData: Partial<TreeNode>) => {
      setNodes(prev => {
        const existingNode = prev.nodes.get(nodeId);
        if (!existingNode) return prev;

        const newNodes = new Map(prev.nodes);
        newNodes.set(nodeId, { ...existingNode, ...nodeData });

        return { ...prev, nodes: newNodes };
      });
    },
    [],
  );

  const handleSetNodes = useCallback((nodesToUpdate: TreeNode[]) => {
    if (nodesToUpdate.length === 0) return;

    setNodes(prev => {
      const newNodes = new Map(prev.nodes);
      const newChildrenMap = new Map(prev.childrenMap);
      const childrenToUpdateMap = new Map<string, string[]>();

      for (const node of nodesToUpdate) {
        const existingNode = newNodes.get(node.id);

        newNodes.set(node.id, {
          ...node,
          isConnected: existingNode?.isConnected ?? node.isConnected,
          isExpanded: existingNode?.isExpanded ?? node.isExpanded,
          hasLoadedChildren:
            existingNode?.hasLoadedChildren ?? node.hasLoadedChildren,
        });

        if (node.parentId) {
          const children = childrenToUpdateMap.get(node.parentId) ?? [];
          children.push(node.id);
          childrenToUpdateMap.set(node.parentId, children);
        }
      }

      childrenToUpdateMap.forEach((childrenIds, parentId) => {
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
    setNodes(prev => {
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
      try {
        const structure = await fetchStructure(serverId, databaseName);

        const formattedStructure = convertDatabaseStructureToNodes(
          serverId,
          databaseName,
          structure,
        );

        handleSetNodes(formattedStructure);
      } catch (error) {
        console.log('handleFetchStructure', error);
        toast.error(
          `Failed to load database structure for ${databaseName}: ${String(
            error,
          )}`,
        );
      }
    },
    [handleSetNodes, fetchStructure],
  );

  const handleLoadDatabaseStructure = useCallback(
    async (node: TreeNode): Promise<TreeNode[]> => {
      const { serverId } = node.metadata;

      switch (node.type) {
        case TreeNodeType.Server: {
          const databases = await getDatabases(serverId);

          return databases.map(db =>
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
        }

        case TreeNodeType.Database: {
          const databaseName = node.name;
          const structure = await fetchStructure(serverId, databaseName);
          return convertDatabaseStructureToNodes(
            serverId,
            databaseName,
            structure,
          );
        }

        case TreeNodeType.Table: {
          const [, tableName, schemaName, databaseName] = node.id.split('-');

          const columns = await fetchColumns(
            serverId,
            databaseName,
            schemaName,
            tableName,
          );

          return columns.map(column =>
            formatTreeNode(
              `column-${column.name}-${tableName}-${schemaName}-${databaseName}-${serverId}`,
              TreeNodeType.Column,
              serverId,
              column.name,
              node.id,
              {
                type: TreeNodeType.Column,
                serverId,
                databaseName,
                dataType: column.dataType,
                isNullable: column.isNullable,
                columnDefault: column.defaultValue ?? null,
              },
            ),
          );
        }

        default:
          return [];
      }
    },
    [],
  );

  const onClickNode = useCallback(
    async (nodeId: string, onClickNodeAction?: (node: TreeNode) => void) => {
      console.log('onClickNode', nodeId);
      const node = nodesRef.current.nodes.get(nodeId);

      if (!node) return;

      const isExpandableType =
        node.type === TreeNodeType.Database ||
        node.type === TreeNodeType.Server ||
        node.type === TreeNodeType.Table;

      if (!isExpandableType) {
        handleSetNode(nodeId, { isExpanded: !node.isExpanded });
        return;
      }

      // If children are already loaded, just toggle expand
      if (node.hasLoadedChildren) {
        handleSetNode(nodeId, { isExpanded: !node.isExpanded });
        onClickNodeAction?.(node);
        return;
      }

      try {
        handleSetNode(nodeId, { isLoading: true });

        const childrenNodes = await handleLoadDatabaseStructure(node);
        handleSetNodes(childrenNodes);

        handleSetNode(nodeId, {
          isLoading: false,
          isExpanded: true,
          hasLoadedChildren: true,
          isConnected: true,
        });
      } catch (error) {
        toast.error('Failed to load data structure.');
        handleSetNode(nodeId, { isLoading: false });
      } finally {
        onClickNodeAction?.(node);
      }
    },
    [handleSetNode, handleSetNodes, handleLoadDatabaseStructure],
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
