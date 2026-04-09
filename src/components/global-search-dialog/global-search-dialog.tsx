import { HugeiconsIcon } from '@hugeicons/react';
import { Search01Icon, TableIcon } from '@hugeicons/core-free-icons';
import {
  CommandDialog,
  CommandEmpty,
  CommandGroup,
  CommandInput,
  CommandItem,
  CommandList,
} from '@/components/ui/command';
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
    groupedResults,
    isLoadingStructures,
    handleSelect,
  } = useGlobalSearchDialog(nodes, onOpenTable);

  return (
    <CommandDialog open={open} onOpenChange={setOpen}>
      <CommandInput
        value={search}
        onValueChange={setSearch}
        placeholder='Search tables (supports "schema/table")'
      />
      <CommandList>
        <CommandEmpty>No table found.</CommandEmpty>
        {groupedResults.map(group => (
          <CommandGroup heading={group.group} key={group.group}>
            {group.items.map(item => (
              <CommandItem
                key={`${item.serverId}-${item.databaseName}-${item.schemaName}-${item.tableName}`}
                value={`${item.serverName} ${item.databaseName} ${item.schemaName} ${item.tableName} ${item.schemaName}/${item.tableName}`}
                onSelect={() => handleSelect(item)}
              >
                <HugeiconsIcon icon={TableIcon} className="size-4" />
                <span>{`${item.schemaName}.${item.tableName}`}</span>
                <span className="ml-auto text-xs text-muted-foreground">
                  {item.databaseName}
                </span>
              </CommandItem>
            ))}
          </CommandGroup>
        ))}
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
      </div>
    </CommandDialog>
  );
};
