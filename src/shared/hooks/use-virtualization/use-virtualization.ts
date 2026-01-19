// src/shared/hooks/use-virtualization.ts
import { useVirtualizer } from "@tanstack/react-virtual";
import { useRef } from "react";

// Hook simplificado para o caso de uso de linhas com altura fixa
export const useVirtualization = (
  itemCount: number,
  estimatedRowHeight: number,
  overscan = 8
) => {
  const parentRef = useRef<HTMLElement>(null);

  const rowVirtualizer = useVirtualizer({
    count: itemCount,
    getScrollElement: () => parentRef.current,
    estimateSize: () => estimatedRowHeight,
    overscan,
  });

  return {
    rowVirtualizer,
    parentRef,
  };
};

export default useVirtualization;
