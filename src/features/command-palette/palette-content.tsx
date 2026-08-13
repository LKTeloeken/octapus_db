import { TableIcon } from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { Search } from 'lucide-react';
import { useRef } from 'react';
import { Badge } from '@/components/ui/badge';
import { Spinner } from '@/components/ui/spinner';
import { DB_TYPE_LABELS } from '@/lib/db-defaults';
import { cn } from '@/lib/utils';
import type { PaletteRow, TableEntry } from './command-palette.types';
import { HighlightedLabel } from './highlighted-label';
import { usePaletteNavigation } from './use-palette-navigation';

const ITEM_HEIGHT = 52;
const HEADER_HEIGHT = 30;
const OVERSCAN = 12;

interface PaletteContentProps {
  query: string;
  setQuery: (value: string) => void;
  rows: PaletteRow[];
  hasResults: boolean;
  connectingId: string | null;
  selectEntry: (entry: TableEntry) => void;
  isEmptyCache: boolean;
}

/**
 * Inner palette UI — input + virtualized result list. Rendered only while the
 * dialog is open (Radix unmounts the content on close), so the virtualizer is
 * created fresh per open, with its lifecycle tied to the scroll element. This
 * avoids a stale virtualizer rendering an empty list on reopen.
 */
export function PaletteContent({
  query,
  setQuery,
  rows,
  hasResults,
  connectingId,
  selectEntry,
  isEmptyCache,
}: PaletteContentProps) {
  const parentRef = useRef<HTMLDivElement>(null);

  const virtualizer = useVirtualizer({
    count: rows.length,
    getScrollElement: () => parentRef.current,
    estimateSize: index =>
      rows[index]?.kind === 'header' ? HEADER_HEIGHT : ITEM_HEIGHT,
    overscan: OVERSCAN,
    getItemKey: index => rows[index]?.key ?? index,
  });

  const { activeIndex, setActiveIndex, onKeyDown } = usePaletteNavigation({
    rows,
    virtualizer,
    onSelect: row => selectEntry(row.item.entry),
  });

  return (
    <div onKeyDown={onKeyDown}>
      <div className="flex h-11 items-center gap-2 border-b px-3">
        <Search className="size-4 shrink-0 opacity-50" />
        <input
          autoFocus
          placeholder="Buscar tabela… (ex.: public.users)"
          value={query}
          onChange={event => setQuery(event.target.value)}
          className="placeholder:text-muted-foreground flex h-11 w-full rounded-md bg-transparent py-3 text-sm outline-hidden"
        />
      </div>

      <div
        ref={parentRef}
        className="max-h-80 overflow-x-hidden overflow-y-auto scrollbar-thin"
      >
        {!hasResults ? (
          <div className="py-6 text-center text-sm text-muted-foreground">
            {isEmptyCache
              ? 'Nenhuma tabela em cache — navegue na árvore para indexá-las.'
              : query.trim()
                ? 'Nenhum resultado.'
                : 'Nenhuma tabela acessada recentemente.'}
          </div>
        ) : (
          <div
            className="relative w-full p-1"
            style={{ height: `${virtualizer.getTotalSize()}px` }}
          >
            {virtualizer.getVirtualItems().map(virtualRow => {
              const row = rows[virtualRow.index];
              if (!row) return null;

              if (row.kind === 'header') {
                return (
                  <div
                    key={virtualRow.key}
                    className="text-muted-foreground absolute left-0 top-0 flex w-full items-end px-2 pb-1.5 text-xs font-medium"
                    style={{
                      height: `${virtualRow.size}px`,
                      transform: `translateY(${virtualRow.start}px)`,
                    }}
                  >
                    {row.heading}
                  </div>
                );
              }

              const { entry, indices, subtitle } = row.item;
              const isConnecting = connectingId === entry.id;
              const isActive = virtualRow.index === activeIndex;

              return (
                <div
                  key={virtualRow.key}
                  role="option"
                  aria-selected={isActive}
                  className={cn(
                    'absolute left-0 top-0 flex w-full cursor-default items-center gap-2 rounded-sm px-2 text-sm select-none',
                    isActive && 'bg-accent text-accent-foreground',
                    isConnecting && 'pointer-events-none opacity-50',
                  )}
                  style={{
                    height: `${virtualRow.size}px`,
                    transform: `translateY(${virtualRow.start}px)`,
                  }}
                  onMouseMove={() => setActiveIndex(virtualRow.index)}
                  onClick={() => !isConnecting && selectEntry(entry)}
                >
                  <HugeiconsIcon
                    icon={TableIcon}
                    className="h-4 w-4 text-muted-foreground"
                  />

                  <div className="flex min-w-0 flex-1 flex-col">
                    <HighlightedLabel text={entry.label} indices={indices} />
                    <span className="truncate text-xs text-muted-foreground">
                      {subtitle}
                    </span>
                  </div>

                  {isConnecting ? (
                    <Spinner className="h-3.5 w-3.5" />
                  ) : (
                    <Badge variant="outline" className="text-[10px]">
                      {DB_TYPE_LABELS[entry.dbType]}
                    </Badge>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>
    </div>
  );
}
