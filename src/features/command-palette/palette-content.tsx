import { Search01Icon, TableIcon } from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { useVirtualizer } from '@tanstack/react-virtual';
import { useRef } from 'react';
import { Badge } from '@/components/ui/badge';
import { Kbd } from '@/components/ui/kbd';
import { Spinner } from '@/components/ui/spinner';
import {
  DB_TYPE_BG_COLOR,
  DB_TYPE_LABELS,
  DB_TYPE_TEXT_COLOR,
} from '@/lib/db-defaults';
import { cn } from '@/lib/utils';
import type { PaletteRow, TableEntry } from './command-palette.types';
import { HighlightedLabel } from './highlighted-label';
import { usePaletteNavigation } from './use-palette-navigation';

const ITEM_HEIGHT = 44;
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
 *
 * Visual (DESIGN.md §7): vidro vem do DialogContent; o item ativo é realce
 * neutro (`--active`), nunca Iris; o ícone da tabela leva a cor do banco.
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

  const resultCount = rows.filter(row => row.kind === 'item').length;

  return (
    <div onKeyDown={onKeyDown}>
      <div className="flex h-14 items-center gap-3 border-b border-line-subtle pr-3.5 pl-[18px]">
        <HugeiconsIcon
          icon={Search01Icon}
          className="size-[18px] shrink-0 text-fg-subtle"
        />
        <input
          autoFocus
          aria-label="Buscar tabela"
          placeholder="Buscar tabela… (ex.: public.users)"
          value={query}
          onChange={event => setQuery(event.target.value)}
          className="h-full w-full bg-transparent text-[16px] text-fg outline-hidden placeholder:text-fg-subtle"
        />
        <Kbd>esc</Kbd>
      </div>

      <div
        ref={parentRef}
        className="max-h-96 overflow-x-hidden overflow-y-auto scrollbar-thin px-2 pt-1 pb-2"
      >
        {!hasResults ? (
          <div className="py-8 text-center text-body text-fg-subtle">
            {isEmptyCache
              ? 'Nenhuma tabela em cache — navegue na árvore para indexá-las.'
              : query.trim()
                ? 'Nenhum resultado.'
                : 'Nenhuma tabela acessada recentemente.'}
          </div>
        ) : (
          <div
            role="listbox"
            aria-label="Resultados"
            className="relative w-full"
            style={{ height: `${virtualizer.getTotalSize()}px` }}
          >
            {virtualizer.getVirtualItems().map(virtualRow => {
              const row = rows[virtualRow.index];
              if (!row) return null;

              if (row.kind === 'header') {
                return (
                  <div
                    key={virtualRow.key}
                    className="absolute top-0 left-0 flex w-full items-end px-2.5 pb-1 text-micro font-medium uppercase text-fg-subtle"
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
                    'absolute top-0 left-0 flex w-full cursor-default items-center gap-2.5 rounded-md px-2.5 select-none',
                    isActive && 'bg-active',
                    isConnecting && 'pointer-events-none opacity-50',
                  )}
                  style={{
                    height: `${virtualRow.size}px`,
                    transform: `translateY(${virtualRow.start}px)`,
                  }}
                  onMouseMove={() => setActiveIndex(virtualRow.index)}
                  onClick={() => !isConnecting && selectEntry(entry)}
                >
                  <span className="flex size-7 shrink-0 items-center justify-center rounded-[7px] bg-hover shadow-[inset_0_0_0_1px_var(--line-subtle)]">
                    <HugeiconsIcon
                      icon={TableIcon}
                      className={cn('size-[15px]', DB_TYPE_TEXT_COLOR[entry.dbType])}
                    />
                  </span>

                  <div className="flex min-w-0 flex-1 flex-col">
                    <span className="truncate text-body">
                      <HighlightedLabel text={entry.label} indices={indices} />
                    </span>
                    {/* No item ativo a legenda sobe um degrau: --fg-subtle
                        sobre vidro + realce fica abaixo de 4,5:1. */}
                    <span
                      className={cn(
                        'truncate text-small',
                        isActive ? 'text-fg-muted' : 'text-fg-subtle',
                      )}
                    >
                      {subtitle}
                    </span>
                  </div>

                  {isConnecting ? (
                    <Spinner className="size-3.5 text-fg-subtle" />
                  ) : isActive ? (
                    <span className="inline-flex shrink-0 items-center gap-1.5 text-small text-fg-muted">
                      Abrir <Kbd>↵</Kbd>
                    </span>
                  ) : (
                    <Badge>
                      <span
                        aria-hidden
                        className={cn(
                          'size-1.5 rounded-full',
                          DB_TYPE_BG_COLOR[entry.dbType],
                        )}
                      />
                      {DB_TYPE_LABELS[entry.dbType]}
                    </Badge>
                  )}
                </div>
              );
            })}
          </div>
        )}
      </div>

      <div className="flex h-[38px] items-center gap-4 border-t border-line-subtle px-4 text-small text-fg-subtle">
        <span className="inline-flex items-center gap-1.5">
          <Kbd>↑</Kbd>
          <Kbd>↓</Kbd>
          navegar
        </span>
        <span className="inline-flex items-center gap-1.5">
          <Kbd>↵</Kbd>
          abrir
        </span>
        <span className="inline-flex items-center gap-1.5">
          <Kbd>esc</Kbd>
          fechar
        </span>
        <span className="flex-1" />
        {hasResults && (
          <span className="tabular-nums">
            {resultCount} resultado{resultCount === 1 ? '' : 's'}
          </span>
        )}
      </div>
    </div>
  );
}
