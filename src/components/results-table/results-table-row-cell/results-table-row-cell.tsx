import { memo } from 'react';
import type { ResultsTableRowCellProps } from './results-table-row-cell.types';
import { cn } from '@/lib/utils';
import { DataTableCell } from '../results-table-cell/results-table-cell';

/**
 * O editor de célula é um Popover portalizado para o `body`, mas eventos de
 * portal sobem pela árvore do **React**, não pela do DOM — um clique dentro do
 * editor chega aqui como se tivesse nascido na linha. Comparar com o DOM real
 * separa os dois: o nó do portal não está contido na linha.
 */
const startedInsideRow = (event: React.MouseEvent<HTMLElement>) =>
  event.currentTarget.contains(event.target as Node);

export const ResultsTableRowCell = memo(
  ({
    row,
    rowIndex,
    isModified,
    isAdded,
    isRemoved,
    isSelected,
    isEven,
    columns,
    columnIndices,
    rowHeight,
    rowStart,
    gutterWidth,
    totalWidth,
    virtualColumns,
    getCellDisplayValue,
    isCellModified,
    isColumnEditable,
    updateCell,
    activeColumnName,
    focusedColumnName,
    onActivateCell,
    onCloseCell,
    onFocusCell,
    onSelectRowBody,
    onSelectRowGutter,
  }: ResultsTableRowCellProps) => {
    // Pending status (red delete > green add > yellow edit) wins over the
    // selection tint; a normal selected row gets the primary tint instead of
    // the zebra striping.
    const rowBg = isRemoved
      ? 'bg-red-900/20'
      : isAdded
        ? 'bg-green-900/20'
        : isModified
          ? 'bg-yellow-900/20'
          : isSelected
            ? 'bg-primary/15'
            : isEven
              ? 'bg-muted'
              : 'bg-transparent';

    // Tint overlays an opaque `bg-sidebar` instead of replacing it: a
    // translucent `bg-primary/30` (or status /60) lets scrolled cell text
    // show through the sticky gutter.
    const gutterTint = isRemoved
      ? 'bg-red-950/60'
      : isAdded
        ? 'bg-green-950/60'
        : isSelected
          ? 'bg-primary/30'
          : null;

    const gutterFg = isRemoved
      ? 'text-red-300'
      : isAdded
        ? 'text-green-300'
        : isSelected
          ? 'text-foreground'
          : 'text-muted-foreground';

    return (
      <div
        onClick={event => {
          if (!startedInsideRow(event)) return;
          onSelectRowBody(rowIndex, event);
        }}
        className={cn(
          'absolute top-0 left-0',
          rowBg,
          // A primary ring always marks the selection, even on status rows.
          isSelected && 'ring-1 ring-inset ring-primary',
        )}
        style={{
          height: `${rowHeight}px`,
          width: `${totalWidth}px`,
          transform: `translateY(${rowStart}px)`,
        }}
      >
        {/* Sticky row identifier doubles as the selection handle */}
        <button
          type="button"
          // A grade tem um único tab stop (o container): com linhas
          // virtualizadas, a ordem natural de Tab seria caótica.
          tabIndex={-1}
          onClick={event => {
            // Don't let the gutter click also trigger the body single-select.
            event.stopPropagation();
            onSelectRowGutter(rowIndex, event);
          }}
          className={cn(
            'sticky left-0 z-20 overflow-hidden border-r border-border flex items-center justify-center',
            'text-[10px] font-mono select-none cursor-pointer outline-none',
            'focus-visible:ring-1 focus-visible:ring-ring',
            'bg-sidebar',
            gutterFg,
          )}
          style={{ width: `${gutterWidth}px`, height: `${rowHeight}px` }}
        >
          {gutterTint && (
            <span
              aria-hidden
              className={cn('absolute inset-0 pointer-events-none', gutterTint)}
            />
          )}
          <span className="relative">{rowIndex + 1}</span>
        </button>

        {virtualColumns.map(virtualColumn => {
          const column = columns[virtualColumn.index];
          if (!column) return null;

          const cell = row[columnIndices[virtualColumn.index]];
          const displayValue = getCellDisplayValue(rowIndex, column.name, cell);
          // Added rows are fully editable (incl. PK); deleted rows are frozen.
          const editable = isRemoved
            ? false
            : isAdded || isColumnEditable(column.name);

          return (
            <div
              key={virtualColumn.key}
              // preventDefault impede que o <button> da célula fique com o foco:
              // ao rolar, a virtualização o desmontaria e o foco cairia no body,
              // matando as setas. Cliques vindos do editor aberto são ignorados
              // — roubar o foco deles fecharia o Popover.
              onMouseDown={event => {
                if (!startedInsideRow(event)) return;
                event.preventDefault();
                onFocusCell(rowIndex, column.name);
              }}
              className={cn(
                'absolute top-0 border-r border-border',
                focusedColumnName === column.name &&
                  'z-10 ring-1 ring-inset ring-primary',
              )}
              style={{
                left: `${gutterWidth + virtualColumn.start}px`,
                width: `${virtualColumn.size}px`,
                height: `${rowHeight}px`,
              }}
            >
              <DataTableCell
                value={cell}
                columnType={column.typeName}
                displayValue={displayValue}
                isModified={isCellModified(rowIndex, column.name)}
                isEditable={editable}
                rowIndex={rowIndex}
                columnName={column.name}
                isActive={activeColumnName === column.name}
                onActivate={onActivateCell}
                onClose={onCloseCell}
                updateCell={updateCell}
              />
            </div>
          );
        })}
      </div>
    );
  },
);
