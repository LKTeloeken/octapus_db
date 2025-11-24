import { memo, useState } from "react";
import { Spinner } from "@/components/ui/spinner";
import { ChevronDown, ChevronRight, MoreHorizontal, Edit } from "lucide-react";
import { useTreeNode } from "./use-tree-node";
import type { TreeNodeProps } from "./tree-node.types";
import { cn } from "@/lib/utils";
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from "@/components/ui/popover";
import { Button } from "@/components/ui/button";

export const TreeNode = memo(
  ({
    nodeId,
    nodes,
    childrenMap,
    onToggle,
    level = 0,
    isLastChild = false,
    onNodeClick,
  }: TreeNodeProps) => {
    const { getNodeIcon } = useTreeNode();
    const node = nodes.get(nodeId);

    if (!node) return null;

    const childrenIds = childrenMap.get(nodeId) || [];
    const hasChildren = node.hasChildren;
    const isExpanded = node.isExpanded;
    const metadata = node.metadata;

    const [isMenuOpen, setIsMenuOpen] = useState(false);

    const handleContextMenu = (e: React.MouseEvent) => {
      if (node.type === "server" && onNodeClick) {
        e.preventDefault();
        e.stopPropagation();
        setIsMenuOpen(true);
      }
    };

    const handleEdit = (e: React.MouseEvent) => {
      e.stopPropagation();
      if (
        onNodeClick &&
        metadata &&
        metadata.type === "server" &&
        metadata.serverData
      ) {
        onNodeClick(metadata.serverData);
      }
    };

    return (
      <div className="relative">
        <div
          className={cn(
            "flex items-center gap-2 py-1 px-2 cursor-pointer hover:bg-accent/50 rounded-md transition-colors",
            "group relative pr-8"
          )}
          style={{ paddingLeft: `${level * 1.25 + 0.5}rem` }}
          onClick={() => hasChildren && onToggle(nodeId)}
          onContextMenu={handleContextMenu}
        >
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

          <div className="flex items-center gap-2">
            {getNodeIcon(node.type)}

            <div className="flex flex-col gap-1 ">
              <div className="text-sm truncate">{node.name}</div>
              {metadata && metadata.type === "column" && (
                <div className="text-xs text-muted-foreground truncate">
                  {metadata.dataType}
                </div>
              )}
            </div>
          </div>

          {node.type === "server" && onNodeClick && (
            <div
              className={cn(
                "absolute right-2 transition-opacity",
                isMenuOpen ? "opacity-100" : "opacity-0 group-hover:opacity-100"
              )}
            >
              <Popover open={isMenuOpen} onOpenChange={setIsMenuOpen}>
                <PopoverTrigger asChild>
                  <Button
                    variant="ghost"
                    size="icon"
                    className="h-6 w-6"
                    onClick={(e) => e.stopPropagation()}
                  >
                    <MoreHorizontal className="h-4 w-4" />
                  </Button>
                </PopoverTrigger>
                <PopoverContent className="w-32 p-1" align="end">
                  <Button
                    variant="ghost"
                    size="sm"
                    className="w-full justify-start gap-2"
                    onClick={(e) => {
                      handleEdit(e);
                      setIsMenuOpen(false);
                    }}
                  >
                    <Edit className="h-3 w-3" />
                    Editar
                  </Button>
                </PopoverContent>
              </Popover>
            </div>
          )}
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
                onNodeClick={onNodeClick}
              />
            ))}
          </div>
        )}

        {/* Linha vertical conectando ao pai */}
        {level > 0 && (
          <div
            className={cn(
              "absolute top-0 w-0.5 bg-secondary",
              isLastChild ? "h-[1.125rem]" : "h-full"
            )}
            style={{ left: `${(level - 1) * 1.25 + 0.95}rem` }}
          />
        )}

        {/* Linha horizontal */}
        {level > 0 && (
          <div
            className="absolute top-[1.125rem] h-0.5 bg-secondary"
            style={{
              left: `${(level - 1) * 1.25 + 0.95}rem`,
              width: "0.5rem",
            }}
          />
        )}
      </div>
    );
  }
);

TreeNode.displayName = "TreeNode";
