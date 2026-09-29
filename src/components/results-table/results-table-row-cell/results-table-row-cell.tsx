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
    // Estado pendente (removida > nova) tinge a linha; célula editada tinge só a
    // célula. A seleção usa `--iris-soft` na linha sem estado e aparece SEMPRE
    // no gutter — o estado pendente nunca a esconde (DESIGN.md §2.4).
    const rowBg = isRemoved
      ? 'bg-danger-soft'
      : isAdded
        ? 'bg-success-soft'
        : isSelected
          ? 'bg-iris-soft'
          : null;

    // O tom do gutter é uma camada sobre o `bg-surface-1` opaco: um fundo
    // translúcido direto deixaria o texto rolado aparecer por trás dele.
    const gutterTint = isSelected
      ? 'bg-iris-soft'
      : isRemoved
        ? 'bg-danger-soft'
        : isAdded
          ? 'bg-success-soft'
          : null;

    // Codificação dupla: além da cor, um glifo (legível para daltônicos).
    const marker = isRemoved
      ? { glyph: '−', color: 'text-danger' }
      : isAdded
        ? { glyph: '+', color: 'text-success' }
        : isModified
          ? { glyph: '•', color: 'text-warning' }
          : null;

    return (
      <div
        onClick={event => {
          if (!startedInsideRow(event)) return;
          onSelectRowBody(rowIndex, event);
        }}
        className={cn('absolute top-0 left-0', rowBg)}
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
            'sticky left-0 z-20 flex items-center justify-between overflow-hidden border-r border-b border-line-subtle bg-surface-1 pr-2 pl-1.5',
            'cursor-pointer select-none font-mono text-[11px] tabular-nums outline-none',
            'focus-visible:ring-1 focus-visible:ring-inset focus-visible:ring-ring',
          )}
          style={{ width: `${gutterWidth}px`, height: `${rowHeight}px` }}
        >
          {gutterTint && (
            <span
              aria-hidden
              className={cn('absolute inset-0 pointer-events-none', gutterTint)}
            />
          )}
          <span
            aria-hidden
            className={cn('relative font-semibold', marker?.color)}
          >
            {marker?.glyph}
          </span>
          <span
            className={cn(
              'relative',
              isSelected ? 'text-iris-text' : 'text-fg-subtle',
            )}
          >
            {rowIndex + 1}
          </span>
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
                'absolute top-0 border-r border-b border-line-subtle',
                // Cursor de teclado: anel inset de 1,5 px, sem fundo.
                focusedColumnName === column.name &&
                  'z-10 ring-[1.5px] ring-inset ring-ring',
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
                isRemoved={isRemoved}
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
