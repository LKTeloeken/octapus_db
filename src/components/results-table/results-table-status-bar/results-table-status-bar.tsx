import { memo, useMemo, useState } from 'react';
import { Spinner } from '@/components/ui/spinner';
import { Button } from '@/components/ui/button';
import { TabType } from '@/shared/models/tabs.types';
import { Checkbox } from '@/components/ui/checkbox';
import { Label } from '@/components/ui/label';
import { Input } from '@/components/ui/input';
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover';

import type { DataTableStatusBarProps } from './results-table-status-bar.types';

export const DataTableStatusBar = memo(
  ({
    executionTimeMs,
    rowCount,
    rowsLength,
    totalCount,
    isEditable,
    editableInfo,
    changesCount,
    isLoadingMore,
    hasMore,
    onDiscardChanges,
    onApplyChanges,
    tabType,
    viewLayout,
    onViewLayoutChange,
    onSwitchToSql,
    columns = [],
    visibleColumnNames = [],
    onToggleColumnVisibility,
    onShowAllColumns,
    onAddRow,
    pendingInsertRowsCount = 0,
    pendingDeleteRowsCount = 0,
  }: DataTableStatusBarProps) => {
    const [columnSearch, setColumnSearch] = useState('');
    const visibleSet = new Set(visibleColumnNames);
    const hiddenCount = Math.max(columns.length - visibleColumnNames.length, 0);
    const filteredColumns = useMemo(
      () =>
        columns.filter(column =>
          column.name.toLowerCase().includes(columnSearch.toLowerCase().trim()),
        ),
      [columnSearch, columns],
    );
    const totalPendingRows = pendingInsertRowsCount + pendingDeleteRowsCount;
    const hasPendingChanges = changesCount > 0 || totalPendingRows > 0;

    return (
      <div className="flex items-center justify-between px-3 py-1.5 border-t border-border bg-purple-glow text-xs text-foreground shrink-0">
        <div className="flex items-center gap-3">
          {executionTimeMs !== undefined && (
            <span>Executado em {executionTimeMs}ms</span>
          )}
          {rowCount !== undefined && (
            <span>
              {rowsLength} linhas carregadas
              {totalCount != null && ` de ${totalCount} total`}
            </span>
          )}
          {isEditable && editableInfo && (
            <span className="text-green-400">
              {editableInfo.schema}.{editableInfo.table}
            </span>
          )}
        </div>
        <div className="flex items-center gap-2">
          {tabType === TabType.View ? (
            <>
              <Button
                type="button"
                variant={viewLayout === 'horizontal' ? 'default' : 'outline'}
                size="sm"
                className="h-6 px-2 text-[11px]"
                onClick={() => onViewLayoutChange?.('horizontal')}
              >
                Horizontal
              </Button>
              <Button
                type="button"
                variant={viewLayout === 'vertical' ? 'default' : 'outline'}
                size="sm"
                className="h-6 px-2 text-[11px]"
                onClick={() => onViewLayoutChange?.('vertical')}
              >
                Vertical
              </Button>
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="h-6 px-2 text-[11px]"
                onClick={onSwitchToSql}
              >
                SQL
              </Button>
              <Button
                type="button"
                variant="outline"
                size="sm"
                className="h-6 px-2 text-[11px]"
                onClick={onAddRow}
              >
                Add row
              </Button>
              {columns.length > 0 && (
                <Popover>
                  <PopoverTrigger asChild>
                    <Button
                      type="button"
                      variant={hiddenCount > 0 ? 'default' : 'outline'}
                      size="sm"
                      className="h-6 px-2 text-[11px]"
                    >
                      Colunas
                      {hiddenCount > 0 ? ` (${hiddenCount} oculta${hiddenCount === 1 ? '' : 's'})` : ''}
                    </Button>
                  </PopoverTrigger>
                  <PopoverContent className="w-72 p-2">
                    <Input
                      value={columnSearch}
                      onChange={event => setColumnSearch(event.target.value)}
                      placeholder="Search columns..."
                      className="mb-2 h-7 text-xs"
                    />
                    <div className="space-y-2 max-h-56 overflow-auto pr-1">
                      {filteredColumns.map(column => {
                        const checked = visibleSet.has(column.name);
                        const inputId = `column-visibility-${column.name}`;
                        return (
                          <div
                            key={column.name}
                            className="flex items-center gap-2"
                          >
                            <Checkbox
                              id={inputId}
                              checked={checked}
                              onCheckedChange={() =>
                                onToggleColumnVisibility?.(column.name)
                              }
                            />
                            <Label
                              htmlFor={inputId}
                              className="text-xs font-normal truncate cursor-pointer"
                            >
                              {column.name}
                            </Label>
                          </div>
                        );
                      })}
                    </div>
                    <div className="pt-2 border-t border-border mt-2 flex justify-end">
                      <Button
                        type="button"
                        variant="ghost"
                        size="sm"
                        className="h-6 px-2 text-[11px]"
                        onClick={onShowAllColumns}
                      >
                        Mostrar todas
                      </Button>
                    </div>
                  </PopoverContent>
                </Popover>
              )}
            </>
          ) : null}
          {hasPendingChanges && (
            <>
              {changesCount > 0 ? (
                <span className="text-yellow-400">
                  {changesCount} alteraç{changesCount === 1 ? 'ão' : 'ões'}{' '}
                  pendente
                  {changesCount === 1 ? '' : 's'}
                </span>
              ) : null}
              <button
                type="button"
                className="px-2 py-0.5 text-xs rounded border border-accent/25 hover:bg-muted/60 transition-colors"
                onClick={onDiscardChanges}
              >
                Descartar
              </button>
              <button
                type="button"
                className="px-2 py-0.5 text-xs rounded border border-accent/25 hover:bg-muted/60 transition-colors"
                onClick={onApplyChanges}
              >
                Aplicar
              </button>
            </>
          )}
          {totalPendingRows > 0 && (
            <span className="text-red-300">
              {pendingInsertRowsCount > 0 ? `${pendingInsertRowsCount} nova${pendingInsertRowsCount === 1 ? '' : 's'}` : ''}
              {pendingInsertRowsCount > 0 && pendingDeleteRowsCount > 0 ? ' / ' : ''}
              {pendingDeleteRowsCount > 0 ? `${pendingDeleteRowsCount} deletar` : ''}
            </span>
          )}
          {isLoadingMore && (
            <>
              <Spinner className="h-3 w-3" />
              <span>Carregando...</span>
            </>
          )}
          {hasMore && !isLoadingMore && (
            <span>Mais resultados disponíveis</span>
          )}
        </div>
      </div>
    );
  },
);
