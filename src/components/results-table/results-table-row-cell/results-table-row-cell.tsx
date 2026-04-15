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
    isSelected,
    isDeleted,
    isInserted,
    visibleColumns,
    rowHeight,
    rowStart,
    onSelect,
    getCellDisplayValue,
    isCellModified,
    isColumnEditable,
    updateCell,
  }: ResultsTableRowCellProps) => {
    return (
      <div
        key={rowIndex}
        className={cn(
          'absolute top-0 left-0 w-full flex items-center transition-colors',
          isDeleted
            ? 'bg-red-900/25'
            : isInserted
              ? 'bg-emerald-900/20'
              : isModified
            ? 'bg-yellow-900/20'
            : isEven
              ? 'bg-muted/30'
              : 'bg-transparent',
          isSelected && 'ring-1 ring-primary/60',
        )}
        style={{
          height: `${rowHeight}px`,
          transform: `translateY(${rowStart}px)`,
        }}
        onClick={onSelect}
      >
        {visibleColumns.map(({ column, columnIndex }) => {
          const cell = row[columnIndex] ?? null;

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
            <div
              key={`cell-${rowIndex}-${column.name}`}
              className="w-48 min-w-48 max-w-48 border-r border-border"
            >
              <DataTableCell
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
