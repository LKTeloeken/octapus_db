import { memo } from 'react';
import type { ColumnCellProps } from './results-table-column-cell.types';
import { cn } from '@/lib/utils';
import { Button } from '@/components/ui/button';
import { HugeiconsIcon } from '@hugeicons/react';
import { ChevronDown } from '@hugeicons/core-free-icons';

const ColumnCell = ({
  column,
  isPrimaryKeyColumn,
  className,
  onReorderTable,
  viewOrder,
}: ColumnCellProps) => {
  return (
    <div
      className={cn(
        'bg-background shadow-[0_1px_0_0_var(--color-border)] h-9 px-2 py-1 text-left align-middle font-medium text-foreground whitespace-nowrap text-xs select-none border-r border-border flex items-center justify-between',
        className,
      )}
    >
      <div className="flex flex-col items-start justify-start">
        <div>{column.name}</div>
        <div className="text-muted-foreground text-xs">
          {column.typeName}
          {isPrimaryKeyColumn ? ' (PK)' : ''}
        </div>
      </div>

      {isPrimaryKeyColumn && (
        <Button
          variant="ghost"
          size="icon"
          className="w-5 h-5"
          onClick={onReorderTable}
        >
          <HugeiconsIcon
            icon={ChevronDown}
            className={cn(
              'w-5 h-5 transition-transform duration-200',
              viewOrder === 'asc' ? 'rotate-180' : '',
            )}
          />
        </Button>
      )}
    </div>
  );
};

export default memo(ColumnCell);
