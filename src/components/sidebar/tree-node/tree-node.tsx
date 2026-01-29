import { memo } from "react";
import { Spinner } from "@/components/ui/spinner";
import {
  ChevronDown,
  ChevronRight,
  MoreHorizontal,
  Edit,
  Plus,
} from "lucide-react";
import { useTreeNode } from "./use-tree-node";
import type { TreeNodeProps } from "./tree-node.types";
import { useStyles } from "./tree-node.styles";
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
    openServerModal,
    openNewTab,
  }: TreeNodeProps) => {
    const node = nodes.get(nodeId);

    if (!node) return null;

    const {
      isMenuOpen,
      childrenIds,
      hasChildren,
      isExpanded,
      metadata,
      getNodeIcon,
      handleContextMenu,
      handleServerEdit,
      setIsMenuOpen,
      handleOpenNewTab,
    } = useTreeNode(node, nodeId, childrenMap, openServerModal, openNewTab);

    const styles = useStyles();

    return (
      <div className={styles.root}>
        <div
          className={styles.container}
          style={{ paddingLeft: `${level * 1.25 + 0.5}rem` }}
          onClick={() => hasChildren && onToggle(nodeId)}
          onContextMenu={handleContextMenu}
        >
          <div className={styles.iconWrapper}>
            {hasChildren && (
              <>
                {node.isLoading ? (
                  <Spinner className={styles.spinner} />
                ) : isExpanded ? (
                  <ChevronDown className={styles.chevronIcon} />
                ) : (
                  <ChevronRight className={styles.chevronIcon} />
                )}
              </>
            )}
          </div>

          <div className={styles.contentWrapper}>
            {getNodeIcon(node.type, !!node.isConnected)}

            <div className={styles.textWrap}>
              <div className={styles.nameText}>{node.name}</div>
              {metadata && metadata.type === "column" && (
                <div className={styles.dataTypeText}>{metadata.dataType}</div>
              )}
            </div>
          </div>

          <div
            className={cn(
              styles.menuButtonWrapper,
              isMenuOpen ? "opacity-100" : "opacity-0 group-hover:opacity-100",
            )}
          >
            <Popover open={isMenuOpen} onOpenChange={setIsMenuOpen}>
              <PopoverTrigger asChild>
                <Button
                  variant="ghost"
                  size="icon"
                  className={styles.menuButton}
                  onClick={(e) => e.stopPropagation()}
                >
                  <MoreHorizontal className="h-4 w-4" />
                </Button>
              </PopoverTrigger>
              <PopoverContent className={styles.popoverContent} align="end">
                {node.type === "server" && (
                  <Button
                    variant="ghost"
                    size="sm"
                    className={styles.menuItemButton}
                    onClick={(e) => {
                      handleServerEdit(e);
                      setIsMenuOpen(false);
                    }}
                  >
                    <Edit className={styles.menuIcon} />
                    Editar
                  </Button>
                )}

                <Button
                  variant="ghost"
                  size="sm"
                  className={styles.menuItemButton}
                  onClick={(e) => {
                    handleOpenNewTab(e);
                    setIsMenuOpen(false);
                  }}
                >
                  <Plus className={styles.menuIcon} />
                  Nova aba
                </Button>
              </PopoverContent>
            </Popover>
          </div>
        </div>

        <div
          className={cn(
            "grid transition-[grid-template-rows] duration-150 ease-in-out",
            isExpanded ? "grid-rows-[1fr]" : "grid-rows-[0fr]",
          )}
        >
          <div
            className={cn(
              "overflow-hidden transition-opacity duration-150 ease-in-out",
              isExpanded ? "opacity-100" : "opacity-0",
            )}
          >
            {childrenIds.map((childId, index) => (
              <TreeNode
                key={childId}
                nodeId={childId}
                nodes={nodes}
                childrenMap={childrenMap}
                onToggle={onToggle}
                level={level + 1}
                isLastChild={index === childrenIds.length - 1}
                openServerModal={openServerModal}
                openNewTab={openNewTab}
              />
            ))}
          </div>
        </div>

        {/* Linha vertical conectando ao pai */}
        {level > 0 && (
          <div
            className={cn(
              styles.verticalLine,
              isLastChild ? "h-[1.125rem]" : "h-full",
            )}
            style={{ left: `${(level - 1) * 1.25 + 0.95}rem` }}
          />
        )}

        {/* Linha horizontal */}
        {level > 0 && (
          <div
            className={styles.horizontalLine}
            style={{
              left: `${(level - 1) * 1.25 + 0.95}rem`,
              width: "0.5rem",
            }}
          />
        )}
      </div>
    );
  },
);

TreeNode.displayName = "TreeNode";
