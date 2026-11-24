import { memo, useMemo } from 'react';
import type { ColumnCellProps } from './results-table-column-cell.types';
import { cn } from '@/lib/utils';
import { HugeiconsIcon } from '@hugeicons/react';
import {
  Key01Icon,
  SortByDown02Icon,
  SortByUp02Icon,
} from '@hugeicons/core-free-icons';
import { isPgArrayType, pgArrayTypeLabel } from '@/lib/pg-array';
import { resolveCellEditor } from '../results-table-cell/use-resolve-cell-editor';

/**
 * Cabeçalho de coluna (40 px): nome 12/500 + tipo em mono. Clicar nele
 * seleciona a coluna (Shift/Cmd soma outras); ordenar é só pelo ícone, que só
 * existe quando a grade ordena (`onSort`). A ordenação ativa é marcada pelo
 * ícone neutro — Tinta não marca coluna (DESIGN.md §2.2). As numéricas alinham
 * à direita, como os valores.
 */
const ColumnCell = ({
  column,
  isPrimaryKeyColumn,
  isSorted,
  sortDirection,
  isSelected = false,
  className,
  onSort,
  onSelect,
}: ColumnCellProps) => {
  const columnTypeLabel = useMemo(() => {
    const isArray = isPgArrayType(column.typeName);

    if (isArray) {
      return pgArrayTypeLabel(column.typeName);
    }

    return column.typeName;
  }, [column.typeName]);

  const isNumeric = resolveCellEditor(column.typeName) === 'number';

  return (
    <div
      onClick={event =>
        onSelect(column.name, event.shiftKey || event.metaKey || event.ctrlKey)
      }
      className={cn(
        'group flex h-full cursor-pointer items-center border-r border-line-subtle px-2.5 select-none',
        // A coluna selecionada usa o mesmo tom das células dela
        isSelected && 'bg-tinta-soft',
        className,
      )}
    >
      <div
        className={cn(
          'flex min-w-0 flex-1 flex-col',
          isNumeric ? 'items-end text-right' : 'items-start text-left',
        )}
      >
        <div className="flex max-w-full items-center gap-1 text-small font-medium text-fg">
          {isPrimaryKeyColumn && (
            <span title="Chave primária" className="flex shrink-0">
              <HugeiconsIcon icon={Key01Icon} className="size-3 text-warning" />
            </span>
          )}
          <span className="truncate">{column.name}</span>
          {/* Ordenação: sólida na coluna ativa, sugerida no hover das demais.
              Botão próprio — o clique no resto do cabeçalho seleciona. */}
          {onSort && (
            <button
              type="button"
              tabIndex={-1}
              aria-label={`Ordenar por ${column.name}`}
              title="Ordenar"
              onClick={event => {
                event.stopPropagation();
                onSort(column.name);
              }}
              className={cn(
                '-my-0.5 flex shrink-0 cursor-pointer items-center rounded-[4px] p-0.5 outline-none transition-[opacity,background-color] hover:bg-hover',
                isSorted
                  ? 'text-tinta-text'
                  : 'text-fg-subtle opacity-0 group-hover:opacity-100',
              )}
            >
              <HugeiconsIcon
                icon={
                  isSorted && sortDirection === 'asc'
                    ? SortByUp02Icon
                    : SortByDown02Icon
                }
                className="size-3.5"
              />
            </button>
          )}
        </div>
        <div className="max-w-full truncate font-mono text-[11px] leading-[14px] text-fg-subtle">
          {columnTypeLabel}
        </div>
      </div>
    </div>
  );
};

export default memo(ColumnCell);
