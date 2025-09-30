// src/components/query-results-table/QueryResultsTable.tsx
import * as React from "react";
import { useVirtualizer } from "@tanstack/react-virtual";
import { cn } from "@/lib/utils"; // Importe sua função `cn` do shadcn

import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";

import type {
  QueryResultsColumn,
  QueryResultsRow,
  QueryResultsTableProps,
} from "./query-results.types";

export function QueryResultsTable({
  columns,
  rows,
  rowHeight = 36,
  overscan = 8,
  className,
  emptyMessage = "Nenhum resultado encontrado.",
}: QueryResultsTableProps) {
  // Referência para o elemento que terá o scroll
  const tableContainerRef = React.useRef<HTMLDivElement>(null);
  console.log("columns", columns);

  // Normaliza as colunas para sempre terem o formato { key, header }
  // Isso simplifica o resto do código.
  const normalizedColumns = React.useMemo<QueryResultsColumn[]>(() => {
    return columns.map((col) =>
      typeof col === "string" ? { key: col, header: col } : col
    );
  }, [columns]);

  const rowVirtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => tableContainerRef.current,
    estimateSize: () => rowHeight, // Altura fixa para cada linha
    overscan,
  });

  console.log("rows", rows);

  console.log("rowVirtualizer", rowVirtualizer.getVirtualItems());

  // Se não houver linhas, exibe a mensagem de "vazio"
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
    <div
      ref={tableContainerRef}
      className={cn(
        "relative h-full w-full overflow-auto rounded-md border",
        className
      )}
    >
      <Table>
        <TableHeader className="sticky top-0 z-20 bg-background shadow-sm">
          <TableRow>
            {normalizedColumns.map((col) => (
              <TableHead key={col.key}>{col.key}</TableHead>
            ))}
          </TableRow>
        </TableHeader>
        <TableBody
          style={{
            height: `${rowVirtualizer.getTotalSize()}px`, // Altura total para criar o espaço de scroll
            position: "relative",
          }}
        >
          {/* Renderiza apenas os itens virtualizados */}
          {rows.map((row, i) => {
            return (
              <TableRow key={i}>
                {normalizedColumns.map((col, i) => (
                  <TableCell key={`${col.key}-${i}`}>
                    {/* Usamos font-mono para dados tabulares, fica mais alinhado */}
                    <span className="font-mono text-sm">
                      {row[col.key] as string}
                    </span>
                  </TableCell>
                ))}
              </TableRow>
            );
          })}
        </TableBody>
      </Table>
    </div>
  );
}
