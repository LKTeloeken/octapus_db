import * as React from "react";
import { useLayoutEffect, useRef, useState, memo } from "react";
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

import type { ResultsTableProps } from "./results-table.types";

export const ResultsTable = memo(function ResultsTable({
  columns,
  rows,
  rowHeight = 36,
  overscan = 8,
  className,
  emptyMessage = "No results found.",
}: ResultsTableProps) {
  const { parentRef, rowVirtualizer } = useVirtualization(
    rows.length,
    rowHeight,
    overscan
  );

  const virtualItems = rowVirtualizer.getVirtualItems();

  // Refs and state for column alignment
  const headerRefs = useRef<(HTMLTableCellElement | null)[]>([]);
  const [colWidths, setColWidths] = useState<number[]>([]);

  // Measure widths after header render (includes resize and column changes)
  useLayoutEffect(() => {
    if (!columns.length) return;
    const widths = headerRefs.current.map(
      (el) => el?.getBoundingClientRect().width || 0
    );
    // Avoid unnecessary re-render if nothing changed
    if (widths.length && widths.some((w, i) => w !== colWidths[i])) {
      setColWidths(widths);
    }
  }, [columns, rows.length]);

  // Recalculate on window resize
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
      containerClassName={cn(
        "relative h-full w-full overflow-auto rounded-md border",
        className
      )}
    >
      {/* Sticky header */}
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

          const isEven = virtualItem.index % 2 === 0;

          return (
            <TableRow
              key={virtualItem.key}
              data-index={virtualItem.index}
              className={cn(
                isEven ? "bg-muted/30" : "bg-transparent",
                "hover:bg-muted/50"
              )}
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
});
