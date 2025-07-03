import React, { useState } from "react";
import useTree, { NestedNode } from "@/shared/hooks/use-tree";
import { Typography } from "@/components/ui/typography";
import { cn } from "@/lib/utils";

import { ITreeNode } from "@/shared/models/tree";

export type renderItemFunction = (
  item: NestedNode,
  onClick: () => void,
  level: number,
  isExpanded: boolean,
  hasChildren: boolean
) => React.ReactNode;

export interface RecursiveListProps {
  /** flat map of node-key → { name, children?: [childKey], data? } */
  tree: Record<string, ITreeNode>;
  /**
   * render one item; receives the nested node and its depth
   * (0 = top-level, 1 = one indent, etc)
   */
  renderItem: renderItemFunction;
  className?: string;
  itemClassName?: string;
  emptyMessage?: string;
}

const RecursiveList = ({
  tree,
  renderItem,
  className,
  itemClassName,
  emptyMessage,
}: RecursiveListProps) => {
  const nested = useTree(tree);

  if (nested.length === 0) {
    return (
      <Typography variant={"p"} className={"font-light"}>
        {emptyMessage || "Nenhum item encontrado."}
      </Typography>
    );
  }

  return (
    <div className={cn("flex flex-col gap-1 p-2", className)}>
      <ItemsRenderer
        items={nested}
        itemClassName={itemClassName}
        renderItem={renderItem}
        level={0}
      />
    </div>
  );
};

interface ItemRendererProps {
  node: NestedNode;
  itemClassName?: string;
  renderItem: renderItemFunction;
  level: number;
}

const ItemRenderer = ({
  node,
  itemClassName,
  renderItem,
  level,
}: ItemRendererProps) => {
  const hasChildren = Boolean(node.children && node.children.length > 0);
  const [isExpanded, setIsExpanded] = useState(false);

  const handleToggle = () => {
    setIsExpanded(!isExpanded);
  };

  return (
    <div className={cn(itemClassName)}>
      <div style={{ paddingLeft: `${level * 10}px` }}>
        {renderItem(node, handleToggle, level, isExpanded, hasChildren)}
      </div>
      {hasChildren && isExpanded && (
        <ItemsRenderer
          items={node.children!}
          itemClassName={itemClassName}
          renderItem={renderItem}
          level={level + 1}
        />
      )}
    </div>
  );
};

const ItemsRenderer = ({
  items,
  itemClassName,
  renderItem,
  level,
}: {
  items: NestedNode[];
  itemClassName?: string;
  renderItem: renderItemFunction;
  level: number;
}) => {
  return (
    <>
      {items.map((node, i) => (
        <ItemRenderer
          key={`${node.name}-${node.type}-${level}-${i}`}
          node={node}
          itemClassName={itemClassName}
          renderItem={renderItem}
          level={level}
        />
      ))}
    </>
  );
};

export default RecursiveList;
