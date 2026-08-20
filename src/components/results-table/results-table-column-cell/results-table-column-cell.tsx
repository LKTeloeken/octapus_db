import { memo } from 'react';
import type { ColumnCellProps } from './results-table-column-cell.types';
import { cn } from '@/lib/utils';
import { HugeiconsIcon } from '@hugeicons/react';
import { ChevronDown } from '@hugeicons/core-free-icons';

const ColumnCell = ({
  column,
  isPrimaryKeyColumn,
  isSorted,
  sortDirection,
  className,
  onSort,
}: ColumnCellProps) => {
  return (
    <div
      onClick={() => onSort(column.name)}
      className={cn(
        'bg-sidebar shadow-[0_1px_0_0_var(--color-border)] h-9 px-2 py-1 text-left align-middle font-medium whitespace-nowrap text-xs select-none border-r border-border flex items-center justify-between cursor-pointer',
        isSorted ? 'bg-primary/10 text-primary' : 'text-foreground',
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

      {/* Sort indicator: solid for the active column, faint otherwise */}
      <HugeiconsIcon
        icon={ChevronDown}
        className={cn(
          'w-4 h-4 shrink-0 transition-transform duration-200',
          isSorted
            ? sortDirection === 'asc'
              ? 'rotate-180 text-primary'
              : 'text-primary'
            : 'opacity-25',
        )}
      />
    </div>
  );
};

export default memo(ColumnCell);
