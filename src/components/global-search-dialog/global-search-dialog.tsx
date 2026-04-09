import { HugeiconsIcon } from '@hugeicons/react';
import { HashtagIcon, TableIcon } from '@hugeicons/core-free-icons';
import { Dialog, DialogContent, DialogTitle } from '@/components/ui/dialog';
import { Input } from '@/components/ui/input';
import type { GlobalSearchDialogProps } from './global-search-dialog.types';
import { useGlobalSearchDialog } from './use-global-search-dialog';
import { Spinner } from '@/components/ui/spinner';

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
    hydratingColumns,
    handleSelect,
  } = useGlobalSearchDialog(nodes, onOpenTable);

  return (
    <Dialog open={open} onOpenChange={setOpen}>
      <DialogContent className="p-0 sm:max-w-2xl" showCloseButton={false}>
        <DialogTitle className="sr-only">Global search</DialogTitle>
        <div className="border-b p-3">
          <Input
            value={search}
            onChange={event => setSearch(event.target.value)}
            placeholder="Search tables and columns..."
            autoFocus
          />
          <div className="text-xs text-muted-foreground mt-2 flex items-center gap-2">
            <span>Cmd/Ctrl + K</span>
            {hydratingColumns ? (
              <>
                <Spinner className="size-3" />
                <span>Hydrating columns...</span>
              </>
            ) : null}
          </div>
        </div>
        <div className="max-h-[400px] overflow-auto p-2">
          {results.length === 0 ? (
            <p className="px-2 py-3 text-sm text-muted-foreground">
              No table or column found
            </p>
          ) : (
            results.map(item => (
              <button
                key={`${item.type}-${item.serverId}-${item.databaseName}-${item.schemaName}-${item.tableName}-${item.columnName ?? ''}`}
                className="w-full rounded-md px-2 py-2 text-left hover:bg-accent flex items-center justify-between gap-2"
                onClick={() => handleSelect(item)}
              >
                <span className="flex items-center gap-2 text-sm">
                  <HugeiconsIcon
                    icon={item.type === 'column' ? HashtagIcon : TableIcon}
                    className="size-4"
                  />
                  {item.type === 'column'
                    ? `${item.columnName} · ${item.schemaName}.${item.tableName}`
                    : `${item.schemaName}.${item.tableName}`}
                </span>
                <span className="text-xs text-muted-foreground">
                  {item.databaseName}
                </span>
              </button>
            ))
          )}
        </div>
      </DialogContent>
    </Dialog>
  );
};
