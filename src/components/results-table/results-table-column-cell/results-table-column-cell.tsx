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
 * Cabeçalho de coluna (40 px): nome 12/500 + tipo em mono. A ordenação ativa
 * é marcada por um ícone neutro — Iris não marca coluna (DESIGN.md §2.2). As
 * numéricas alinham à direita, como os valores.
 */
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

  const isNumeric = resolveCellEditor(column.typeName) === 'number';

  return (
    <div
      onClick={() => onSort(column.name)}
      className={cn(
        'group flex h-full cursor-pointer items-center border-r border-line-subtle px-2.5 select-none',
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
          {/* Ordenação: sólida na coluna ativa, sugerida no hover das demais */}
          <HugeiconsIcon
            icon={
              isSorted && sortDirection === 'asc'
                ? SortByUp02Icon
                : SortByDown02Icon
            }
            className={cn(
              'size-3.5 shrink-0 transition-opacity',
              isSorted
                ? 'text-fg'
                : 'text-fg-subtle opacity-0 group-hover:opacity-100',
            )}
          />
        </div>
        <div className="max-w-full truncate font-mono text-[11px] leading-[14px] text-fg-subtle">
          {columnTypeLabel}
        </div>
      </div>
    </div>
  );
};

export default memo(ColumnCell);
