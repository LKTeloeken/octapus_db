// src/components/query-results-table/QueryResultsTable.tsx
import * as React from "react";
import { useLayoutEffect, useRef, useState } from "react";
import useVirtualization from "@/shared/hooks/use-virtualization";
import { cn } from "@/lib/utils";

import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

import type { QueryResultsTableProps } from "./query-results.types";

export function QueryResultsTable({
  columns,
  rows,
  rowHeight = 36,
  overscan = 8,
  className,
  emptyMessage = "Nenhum resultado encontrado.",
}: QueryResultsTableProps) {
  const { parentRef, rowVirtualizer } = useVirtualization(
    rows.length,
    rowHeight,
    overscan
  );

  const virtualItems = rowVirtualizer.getVirtualItems();

  // Refs e estado para alinhamento de colunas
  const headerRefs = useRef<(HTMLTableCellElement | null)[]>([]);
  const [colWidths, setColWidths] = useState<number[]>([]);

  // Mede larguras após renderização do header (inclui resize e mudança de colunas)
  useLayoutEffect(() => {
    if (!columns.length) return;
    const widths = headerRefs.current.map(
      (el) => el?.getBoundingClientRect().width || 0
    );
    // Evita re-render desnecessário se nada mudou
    if (widths.length && widths.some((w, i) => w !== colWidths[i])) {
      setColWidths(widths);
    }
  }, [columns, rows.length]);

  // Recalcular em resize da janela
  useLayoutEffect(() => {
    const handle = () => {
      const widths = headerRefs.current.map(
        (el) => el?.getBoundingClientRect().width || 0
      );
      setColWidths(widths);
    };
    window.addEventListener("resize", handle);
    return () => window.removeEventListener("resize", handle);
  }, []);

  if (rows.length === 0) {
    return (
      <div
        className={cn(
          "flex h-48 items-center justify-center rounded-md border",
          className
        )}
      >
        <p className="text-muted-foreground">{emptyMessage}</p>
      </div>
    );
  }

  return (
    <Table
      ref={parentRef as React.RefObject<HTMLDivElement>}
      // Classes aplicadas ao container scroll
      containerClassName={cn(
        "relative h-full w-full overflow-auto rounded-md border",
        className
      )}
    >
      {/* Header sticky - usar background para não vazar conteúdo */}
      <TableHeader className="sticky top-0 z-20 bg-background shadow-sm">
        <TableRow>
          {columns.map((col, idx) => (
            <TableHead
              key={col}
              ref={(el) => {
                headerRefs.current[idx] = el;
              }}
              style={colWidths[idx] ? { width: colWidths[idx] } : undefined}
            >
              {col}
            </TableHead>
          ))}
        </TableRow>
      </TableHeader>

      <TableBody
        style={{
          height: `${rowVirtualizer.getTotalSize()}px`,
          position: "relative",
        }}
      >
        {virtualItems.map((virtualItem) => {
          const row = rows[virtualItem.index];
          if (!row) return null;

          return (
            <TableRow
              key={virtualItem.key}
              data-index={virtualItem.index}
              style={{
                position: "absolute",
                top: 0,
                left: 0,
                width: "100%",
                height: `${rowHeight}px`,
                transform: `translateY(${virtualItem.start}px)`,
              }}
            >
              {columns.map((col, cIdx) => {
                const width = colWidths[cIdx];
                return (
                  <TableCell
                    key={`${col}-${virtualItem.index}`}
                    style={
                      width
                        ? { width, minWidth: width, maxWidth: width }
                        : undefined
                    }
                  >
                    <span className="font-mono text-xs truncate inline-block w-full">
                      {String(row[col] ?? "")}
                    </span>
                  </TableCell>
                );
              })}
            </TableRow>
          );
        })}
      </TableBody>
    </Table>
  );
}
