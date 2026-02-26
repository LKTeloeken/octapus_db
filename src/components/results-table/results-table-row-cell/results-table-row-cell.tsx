import { memo } from 'react';
import type { ResultsTableRowCellProps } from './results-table-row-cell.types';
import { cn } from '@/lib/utils';
import { DataTableCell } from '../results-table-cell/results-table-cell';

export const ResultsTableRowCell = memo(
  ({
    row,
    rowIndex,
    isModified,
    isEven,
    columns,
    rowHeight,
    rowStart,
    getCellDisplayValue,
    isCellModified,
    isColumnEditable,
    updateCell,
  }: ResultsTableRowCellProps) => {
    return (
      <div
        key={rowIndex}
        className={cn(
          'absolute top-0 left-0 w-full flex items-center',
          isModified
            ? 'bg-yellow-900/20'
            : isEven
              ? 'bg-muted/30'
              : 'bg-transparent',
        )}
        style={{
          height: `${rowHeight}px`,
          transform: `translateY(${rowStart}px)`,
        }}
      >
        {row.map((cell, cellIndex) => {
          const column = columns[cellIndex];

          const columnType = column?.typeName;
          const displayValue = getCellDisplayValue(
            rowIndex,
            column?.name ?? '',
            cell,
          );
          const isModified = isCellModified(rowIndex, column?.name ?? '');
          const isEditable = isColumnEditable(column?.name ?? '');
          const onSave = (newValue: string) => {
            updateCell(rowIndex, column?.name ?? '', cell, newValue);
          };

          return (
            <div className="w-48 min-w-48 max-w-48 border-r border-border">
              <DataTableCell
                key={`${rowIndex}-${cellIndex}`}
                value={cell}
                columnType={columnType}
                displayValue={displayValue}
                isModified={isModified}
                isEditable={isEditable}
                onSave={onSave}
              />
            </div>
          );
        })}
      </div>
    );
  },
);
