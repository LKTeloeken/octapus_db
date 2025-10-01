import { Virtualizer } from "@tanstack/react-virtual";

export type UseVirtualization = <T extends { id: string | number }>(
  list: Array<T>,
  estimatedRowHeight: number,
  overscan?: number,
  expandedItems?: Array<number | string>,
  gap?: number
) => {
  rowVirtualizer: Virtualizer<HTMLElement, Element>;
  parentRef: React.RefObject<HTMLElement | null>;
};
