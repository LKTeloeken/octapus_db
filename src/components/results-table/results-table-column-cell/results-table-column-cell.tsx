import { memo } from 'react';
import type { ColumnCellProps } from './results-table-column-cell.types';
import { cn } from '@/lib/utils';

const ColumnCell = ({
  column,
  isPrimaryKeyColumn,
  className,
}: ColumnCellProps) => {
  return (
    <div
      className={cn(
        'bg-background shadow-[0_1px_0_0_var(--color-border)] h-9 px-2 py-1 text-left align-middle font-medium text-foreground whitespace-nowrap text-xs select-none border-r border-border flex flex-col items-start justify-start',
        className,
      )}
    >
      <div>{column.name}</div>
      <div className="text-muted-foreground text-xs">
        {column.typeName}
        {isPrimaryKeyColumn ? ' (PK)' : ''}
      </div>
    </div>
  );
};

export default memo(ColumnCell);
