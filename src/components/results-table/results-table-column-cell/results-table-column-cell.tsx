import { HugeiconsIcon } from '@hugeicons/react';
import {
  ArrowDown01Icon,
  ArrowUp01Icon,
  ArrowUpRight01Icon,
} from '@hugeicons/core-free-icons';
import { memo } from 'react';
import type { ColumnCellProps } from './results-table-column-cell.types';
import { cn } from '@/lib/utils';
import { Button } from '@/components/ui/button';

const ColumnCell = ({
  column,
  isPrimaryKeyColumn,
  className,
  sortable = false,
  sort,
  onSortColumn,
  onOpenForeignTable,
}: ColumnCellProps) => {
  const isSorted = sort?.column === column.name;

  return (
    <div
      className={cn(
        'bg-background shadow-[0_1px_0_0_var(--color-border)] h-9 px-2 py-1 text-left align-middle font-medium text-foreground whitespace-nowrap text-xs select-none border-r border-border flex flex-col items-start justify-start',
        className,
      )}
    >
      <div className="w-full flex items-center justify-between gap-1">
        <Button
          variant="ghost"
          size="sm"
          className={cn(
            'h-auto px-0 py-0 text-xs font-medium justify-start hover:bg-transparent',
            !sortable && 'pointer-events-none',
          )}
          onClick={() => onSortColumn?.(column.name)}
        >
          {column.name}
          {isSorted ? (
            <HugeiconsIcon
              icon={sort.direction === 'asc' ? ArrowUp01Icon : ArrowDown01Icon}
              className="size-3 ml-1"
            />
          ) : null}
        </Button>

        {column.foreignKeyTarget && (
          <Button
            variant="ghost"
            size="icon"
            className="h-4 w-4"
            onClick={() => {
              const target = column.foreignKeyTarget;
              if (!target) return;
              onOpenForeignTable?.(target);
            }}
          >
            <HugeiconsIcon icon={ArrowUpRight01Icon} className="size-3" />
          </Button>
        )}
      </div>
      <div className="text-muted-foreground text-xs truncate w-full">
        {column.typeName}
        {isPrimaryKeyColumn ? ' (PK)' : ''}
      </div>
    </div>
  );
};

export default memo(ColumnCell);
