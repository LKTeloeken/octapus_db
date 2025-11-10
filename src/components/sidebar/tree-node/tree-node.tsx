import { memo } from "react";
import { Spinner } from "@/components/ui/spinner";
import { ChevronDown, ChevronRight } from "lucide-react";
import { useTreeNode } from "./use-tree-node";
import type { TreeNodeProps } from "./tree-node.types";

export const TreeNode = memo(
  ({ nodeId, nodes, childrenMap, onToggle }: TreeNodeProps) => {
    const { getNodeIcon } = useTreeNode();
    const node = nodes.get(nodeId);
    if (!node) return null;

    const childrenIds = childrenMap.get(nodeId) || [];
    const hasChildren = node.hasChildren;
    const isExpanded = node.isExpanded;

    console.log("node IsExpanded" + nodeId, isExpanded);

    return (
      <div className="ml-4">
        <div
          className="flex items-center gap-2 py-1 cursor-pointer dark:hover:bg-gray-800 rounded-lg"
          onClick={() => hasChildren && onToggle(nodeId)}
        >
          {/* Ícone de expansão */}
          {hasChildren && (
            <span className="w-4">
              {node.isLoading ? (
                <Spinner />
              ) : isExpanded ? (
                <ChevronDown />
              ) : (
                <ChevronRight />
              )}
            </span>
          )}

          {/* Ícone do tipo */}
          <span>{getNodeIcon(node.type)}</span>

          {/* Nome do node */}
          <span>{node.name}</span>
        </div>

        {/* Renderiza filhos apenas se expandido */}
        {isExpanded && childrenIds.length > 0 && (
          <div>
            {childrenIds.map((childId) => (
              <TreeNode
                key={childId}
                nodeId={childId}
                nodes={nodes}
                childrenMap={childrenMap}
                onToggle={onToggle}
              />
            ))}
          </div>
        )}
      </div>
    );
  }
);

TreeNode.displayName = "TreeNode";
