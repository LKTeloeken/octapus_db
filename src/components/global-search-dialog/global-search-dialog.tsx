import { HugeiconsIcon } from '@hugeicons/react';
import { Search01Icon, TableIcon } from '@hugeicons/core-free-icons';
import {
  CommandDialog,
  CommandEmpty,
  CommandInput,
  CommandItem,
  CommandList,
} from '@/components/ui/command';
import type { GlobalSearchDialogProps } from './global-search-dialog.types';
import { useGlobalSearchDialog } from './use-global-search-dialog';
import { Spinner } from '@/components/ui/spinner';
import { useVirtualizer } from '@tanstack/react-virtual';
import { useEffect, useMemo, useRef, useState, type KeyboardEvent } from 'react';

export const GlobalSearchDialog = ({
  nodes,
  onOpenTable,
}: GlobalSearchDialogProps) => {
  const {
    open,
    setOpen,
    search,
    setSearch,
    results,
    groupedResults,
    isLoadingStructures,
    isOpeningTable,
    handleSelect,
  } = useGlobalSearchDialog(nodes, onOpenTable);

  const [activeResultIndex, setActiveResultIndex] = useState(0);
  const listRef = useRef<HTMLDivElement>(null);

  const flattenedRows = useMemo(() => {
    const rows: Array<
      | { type: 'group'; key: string; label: string }
      | { type: 'item'; key: string; itemIndex: number; groupLabel: string }
    > = [];
    let itemIndex = 0;

    groupedResults.forEach(group => {
      rows.push({
        type: 'group',
        key: `group-${group.group}`,
        label: group.group,
      });

      group.items.forEach(item => {
        rows.push({
          type: 'item',
          key: `item-${item.serverId}-${item.databaseName}-${item.schemaName}-${item.tableName}`,
          itemIndex,
          groupLabel: group.group,
        });
        itemIndex += 1;
      });
    });

    return rows;
  }, [groupedResults]);

  const flatIndexByItemIndex = useMemo(() => {
    const map = new Map<number, number>();
    flattenedRows.forEach((row, rowIndex) => {
      if (row.type === 'item') {
        map.set(row.itemIndex, rowIndex);
      }
    });
    return map;
  }, [flattenedRows]);

  const rowVirtualizer = useVirtualizer({
    count: flattenedRows.length,
    getScrollElement: () => listRef.current,
    estimateSize: index => (flattenedRows[index]?.type === 'group' ? 28 : 36),
    overscan: 8,
  });

  useEffect(() => {
    if (!open) return;
    setActiveResultIndex(0);
    rowVirtualizer.scrollToOffset(0);
  }, [open, search, rowVirtualizer]);

  useEffect(() => {
    if (results.length === 0) {
      setActiveResultIndex(0);
      return;
    }

    setActiveResultIndex(prev => Math.min(prev, results.length - 1));
  }, [results.length]);

  const handleNavigate = (nextIndex: number) => {
    if (results.length === 0) return;

    const bounded = Math.min(Math.max(nextIndex, 0), results.length - 1);
    setActiveResultIndex(bounded);
    const flatIndex = flatIndexByItemIndex.get(bounded);
    if (typeof flatIndex === 'number') {
      rowVirtualizer.scrollToIndex(flatIndex, { align: 'auto' });
    }
  };

  const handleInputKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    if (event.key === 'ArrowDown') {
      event.preventDefault();
      handleNavigate(activeResultIndex + 1);
      return;
    }

    if (event.key === 'ArrowUp') {
      event.preventDefault();
      handleNavigate(activeResultIndex - 1);
      return;
    }

    if (event.key === 'Enter') {
      event.preventDefault();
      const selected = results[activeResultIndex];
      if (selected) {
        void handleSelect(selected);
      }
    }
  };

  return (
    <CommandDialog open={open} onOpenChange={setOpen}>
      <CommandInput
        value={search}
        onValueChange={setSearch}
        onKeyDown={handleInputKeyDown}
        placeholder='Search tables (supports "schema/table")'
      />
      <CommandList className="h-[380px] p-0" ref={listRef}>
        <CommandEmpty>No table found.</CommandEmpty>
        {results.length > 0 ? (
          <div
            className="relative w-full"
            style={{ height: `${rowVirtualizer.getTotalSize()}px` }}
          >
            {rowVirtualizer.getVirtualItems().map(virtualRow => {
              const row = flattenedRows[virtualRow.index];
              if (!row) return null;

              if (row.type === 'group') {
                return (
                  <div
                    key={row.key}
                    className="absolute left-0 top-0 w-full px-2 py-1.5 text-xs text-muted-foreground font-medium"
                    style={{ transform: `translateY(${virtualRow.start}px)` }}
                  >
                    {row.label}
                  </div>
                );
              }

              const item = results[row.itemIndex];
              if (!item) return null;
              const selected = row.itemIndex === activeResultIndex;

              return (
                <div
                  key={row.key}
                  className="absolute left-0 top-0 w-full px-1"
                  style={{ transform: `translateY(${virtualRow.start}px)` }}
                >
                  <CommandItem
                    value={`${item.serverName} ${item.databaseName} ${item.schemaName} ${item.tableName} ${item.schemaName}.${item.tableName} ${item.schemaName}/${item.tableName}`}
                    className={selected ? 'bg-accent text-accent-foreground' : ''}
                    onMouseMove={() => setActiveResultIndex(row.itemIndex)}
                    onSelect={() => handleSelect(item)}
                  >
                    <HugeiconsIcon icon={TableIcon} className="size-4" />
                    <span>{`${item.schemaName}.${item.tableName}`}</span>
                    <span className="ml-auto text-xs text-muted-foreground">
                      {item.databaseName}
                    </span>
                  </CommandItem>
                </div>
              );
            })}
          </div>
        ) : null}
      </CommandList>
      <div className="border-t px-3 py-2 text-xs text-muted-foreground flex items-center gap-2">
        <HugeiconsIcon icon={Search01Icon} className="size-3" />
        <span>Use ↑ ↓ and Enter to open a table</span>
        {isLoadingStructures ? (
          <>
            <Spinner className="size-3" />
            <span>Loading servers...</span>
          </>
        ) : null}
        {isOpeningTable ? (
          <>
            <Spinner className="size-3" />
            <span>Opening table...</span>
          </>
        ) : null}
      </div>
    </CommandDialog>
  );
};
