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
import { Spinner } from '@/components/ui/spinner';
import type { GlobalSearchDialogProps, SearchTarget } from './global-search-dialog.types';
import { useGlobalSearchDialog } from './use-global-search-dialog';

const COMMAND_LIST_MAX_HEIGHT = 380;

function ResultsList({
  groupedResults,
  onSelect,
}: {
  groupedResults: Array<{ group: string; items: SearchTarget[] }>;
  onSelect: (item: SearchTarget) => Promise<void>;
}) {
  return (
    <>
      <CommandEmpty>No table found.</CommandEmpty>
      {groupedResults.map(group => (
        <CommandGroup key={group.group} heading={group.group}>
          {group.items.map(item => (
            // cmdk expects CommandItem to stay in its own list/group hierarchy.
            <CommandItem
              key={`${item.serverId}-${item.databaseName}-${item.schemaName}-${item.tableName}`}
              value={`${item.serverName} ${item.databaseName} ${item.schemaName}.${item.tableName}`}
              onSelect={() => {
                void onSelect(item);
              }}
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
    </>
  );
}

function ResultsFooter({
  isLoadingStructures,
  isOpeningTable,
}: {
  isLoadingStructures: boolean;
  isOpeningTable: boolean;
}) {
  return (
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
  );
}

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
    isOpeningTable,
    handleSelect,
  } = useGlobalSearchDialog(nodes, onOpenTable);

  return (
    <CommandDialog open={open} onOpenChange={setOpen}>
      <CommandInput
        value={search}
        onValueChange={setSearch}
        placeholder='Search tables (supports "schema/table")'
      />
      <CommandList
        className="p-0"
        style={{ maxHeight: COMMAND_LIST_MAX_HEIGHT, height: 'min(50vh, 380px)' }}
      >
        <ResultsList groupedResults={groupedResults} onSelect={handleSelect} />
      </CommandList>
      <ResultsFooter
        isLoadingStructures={isLoadingStructures}
        isOpeningTable={isOpeningTable}
      />
    </CommandDialog>
  );
};
