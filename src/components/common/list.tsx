import React, { Fragment } from "react";
import { Typography } from "@/components/ui/typography";

interface ListProps<T> {
  items: T[];
  renderItem: (item: T, index: number) => React.ReactNode;
  className?: string;
  itemClassName?: string;
  emptyMessage?: string;
}

const List = <T,>({
  items,
  renderItem,
  className,
  emptyMessage,
}: ListProps<T>) => {
  if (items.length === 0) {
    return (
      <Typography variant="p" className={"font-light"}>
        {emptyMessage || "Nenhum item encontrado."}
      </Typography>
    );
  }

  return (
    <div className={className}>
      {items.map((item, index) => (
        <Fragment key={index}>{renderItem(item, index)}</Fragment>
      ))}
    </div>
  );
};

export default List;
