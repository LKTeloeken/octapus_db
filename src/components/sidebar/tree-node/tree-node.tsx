import { memo } from "react";
import { Spinner } from "@/components/ui/spinner";
import { ChevronDown, ChevronRight } from "lucide-react";
import { useTreeNode } from "./use-tree-node";
import type { TreeNodeProps } from "./tree-node.types";
import { cn } from "@/lib/utils";

export const TreeNode = memo(
  ({
    nodeId,
    nodes,
    childrenMap,
    onToggle,
    level = 0,
    isLastChild = false,
  }: TreeNodeProps) => {
    const { getNodeIcon } = useTreeNode();
    const node = nodes.get(nodeId);
    if (!node) return null;

    const childrenIds = childrenMap.get(nodeId) || [];
    const hasChildren = node.hasChildren;
    const isExpanded = node.isExpanded;

    return (
      <div className="relative">
        {/* Linha vertical conectando ao pai */}
        {level > 0 && (
          <div
            className={cn(
              "absolute left-0 top-0 w-0.5 bg-secondary",
              isLastChild ? "h-[1.125rem]" : "h-full"
            )}
            style={{ left: `${(level - 1) * 1.25}rem` }}
          />
        )}

        {/* Linha horizontal */}
        {level > 0 && (
          <div
            className="absolute top-[1.125rem] h-0.5 bg-secondary"
            style={{
              left: `${(level - 1) * 1.25}rem`,
              width: "1rem",
            }}
          />
        )}

        <div
          className={cn(
            "flex items-center gap-2 py-1 px-2 cursor-pointer hover:bg-accent/50 rounded-md transition-colors",
            "group relative"
          )}
          style={{ paddingLeft: `${level * 1.25 + 0.5}rem` }}
          onClick={() => hasChildren && onToggle(nodeId)}
        >
          {/* Ícone de expansão */}
          <div className="flex items-center justify-center w-4 h-4 shrink-0">
            {hasChildren && (
              <>
                {node.isLoading ? (
                  <Spinner className="w-3 h-3" />
                ) : isExpanded ? (
                  <ChevronDown className="w-4 h-4 text-muted-foreground" />
                ) : (
                  <ChevronRight className="w-4 h-4 text-muted-foreground" />
                )}
              </>
            )}
          </div>

          {/* Ícone do tipo */}
          <span className="flex items-center justify-center shrink-0 text-muted-foreground">
            {getNodeIcon(node.type)}
          </span>

          {/* Nome do node */}
          <span className="text-sm truncate">{node.name}</span>
        </div>

        {/* Renderiza filhos apenas se expandido */}
        {isExpanded && childrenIds.length > 0 && (
          <div>
            {childrenIds.map((childId, index) => (
              <TreeNode
                key={childId}
                nodeId={childId}
                nodes={nodes}
                childrenMap={childrenMap}
                onToggle={onToggle}
                level={level + 1}
                isLastChild={index === childrenIds.length - 1}
              />
            ))}
          </div>
        )}
      </div>
    );
  }
);

TreeNode.displayName = "TreeNode";
