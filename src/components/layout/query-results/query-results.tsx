import React from "react";
import {
  Table,
  TableBody,
  TableCell,
  TableHead,
  TableHeader,
  TableRow,
} from "@/components/ui/table";
import type {
  QueryResultsTableProps,
  QueryResultsColumn,
} from "./query-results.types";

// Small hook to observe element size
function useSize(elRef: React.RefObject<HTMLElement>) {
  const [size, setSize] = React.useState({ width: 0, height: 0 });
  React.useLayoutEffect(() => {
    const el = elRef.current;
    if (!el) return;
    const ro = new ResizeObserver((entries) => {
      const cr = entries[0]?.contentRect;
      if (cr) setSize({ width: cr.width, height: cr.height });
    });
    ro.observe(el);
    return () => ro.disconnect();
  }, [elRef]);
  return size;
}

function normalizeColumns(
  columns: QueryResultsTableProps["columns"]
): QueryResultsColumn[] {
  return columns.map((c) =>
    typeof c === "string" ? { key: c, label: c } : { label: c.key, ...c }
  );
}

export default function QueryResultsTable(props: QueryResultsTableProps) {
  const {
    columns: rawColumns,
    rows,
    rowHeight = 36,
    overscan = 8,
    className,
    emptyMessage = "Nenhum resultado ainda",
  } = props;

  const columns = React.useMemo(
    () => normalizeColumns(rawColumns),
    [rawColumns]
  );

  const scrollRef = React.useRef<HTMLDivElement>(null);
  const { height: viewportHeight } = useSize(
    scrollRef as React.RefObject<HTMLElement>
  );
  const [scrollTop, setScrollTop] = React.useState(0);

  const total = rows.length;
  const totalHeight = total * rowHeight;
  const viewportRows = Math.ceil(viewportHeight / rowHeight) || 0;

  const start = Math.max(0, Math.floor(scrollTop / rowHeight) - overscan);
  const end = Math.min(total, start + viewportRows + overscan * 2);
  const offsetY = start * rowHeight;

  const onScroll = React.useCallback((e: React.UIEvent<HTMLDivElement>) => {
    setScrollTop(e.currentTarget.scrollTop);
  }, []);

  return (
    <div
      ref={scrollRef}
      onScroll={onScroll}
      className={
        "h-full w-full overflow-auto p-4 " +
        (className ? String(className) : "")
      }
    >
      <Table className="w-full" style={{ tableLayout: "fixed" }}>
        <TableHeader>
          <TableRow>
            {columns.map((col) => (
              <TableHead
                key={col.key}
                style={{
                  width: col.width as any,
                  textAlign: col.align ?? "left",
                }}
                className="truncate"
                title={col.label ?? col.key}
              >
                {col.label ?? col.key}
              </TableHead>
            ))}
          </TableRow>
        </TableHeader>

        <TableBody>
          {total === 0 ? (
            <TableRow>
              <TableCell colSpan={columns.length} className="py-10 text-center">
                {emptyMessage}
              </TableCell>
            </TableRow>
          ) : (
            <>
              {/* Spacer row to create the correct total scroll height */}
              {/* We use a single row with relative container and absolutely positioned visible rows */}
              {/* <tr>
                <td colSpan={columns.length} style={{ padding: 0, border: 0 }}>
                  <div style={{ position: "relative", height: totalHeight }}>
                    <div
                      style={{
                        position: "absolute",
                        top: offsetY,
                        left: 0,
                        right: 0,
                        display: "flex",
                      }}
                    > */}
              {rows.slice(start, end).map((row, i) => {
                const rowIndex = start + i;
                return (
                  <TableRow
                    key={rowIndex}
                    style={{ height: rowHeight, width: "100%" }}
                  >
                    {columns.map((c) => {
                      const value = row[c.key];
                      return (
                        <TableCell
                          key={c.key}
                          style={{ textAlign: c.align ?? "left" }}
                          className="truncate"
                          title={value == null ? "" : String(value)}
                        >
                          {value as React.ReactNode}
                        </TableCell>
                      );
                    })}
                  </TableRow>
                );
              })}
              {/* </div>
                  </div>
                </td>
              </tr> */}
            </>
          )}
        </TableBody>
      </Table>
    </div>
  );
}
