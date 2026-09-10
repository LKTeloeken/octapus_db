import { memo, useMemo } from 'react';
import type { ColumnCellProps } from './results-table-column-cell.types';
import { cn } from '@/lib/utils';
import { HugeiconsIcon } from '@hugeicons/react';
import { ChevronDown } from '@hugeicons/core-free-icons';
import { isPgArrayType, pgArrayTypeLabel } from '@/lib/pg-array';

const ColumnCell = ({
  column,
  isPrimaryKeyColumn,
  isSorted,
  sortDirection,
  className,
  onSort,
}: ColumnCellProps) => {
  const columnTypeLabel = useMemo(() => {
    const isArray = isPgArrayType(column.typeName);

    if (isArray) {
      return pgArrayTypeLabel(column.typeName);
    }

    return column.typeName;
  }, [column.typeName]);

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
          {columnTypeLabel}
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
