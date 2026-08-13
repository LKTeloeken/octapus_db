import { memo, useCallback, useMemo } from 'react';
import { ColumnSelector } from '@/components/column-selector/column-selector';
import { useHiddenColumnsReset } from '@/components/column-selector/use-hidden-columns-reset';
import { QueryEditor } from '@/components/query-editor/query-editor/query-editor';
import { ResultsTable } from '@/components/results-table/results-table';
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from '@/components/ui/resizable';
import { useTabsStore } from '@/stores/tabs-store';
import { useQueryRunner } from './use-query-runner';
import type { QueryEditorPanelProps } from './query-editor-panel.types';

/**
 * Free-editor tab: native syntax per database (SQL / Mongo shell / Redis).
 * Dialect and placeholder adapt through get_capabilities (BACKEND.md §5).
 */
export const QueryEditorPanel = memo(({ tab }: QueryEditorPanelProps) => {
  const {
    result,
    isRunning,
    isLoadingMore,
    supportsSql,
    placeholder,
    sqlCompletion,
    setContent,
    executeRun,
    loadMore,
    save,
  } = useQueryRunner(tab);

  const setQueryHiddenColumns = useTabsStore(
    state => state.setQueryHiddenColumns,
  );

  const columns = useMemo(() => result?.columns ?? [], [result]);
  const hiddenColumns = useMemo(
    () => new Set(tab.hiddenColumns),
    [tab.hiddenColumns],
  );
  const setHiddenColumns = useCallback(
    (cols: string[]) => setQueryHiddenColumns(tab.id, cols),
    [tab.id, setQueryHiddenColumns],
  );

  // Outra query (conjunto de colunas diferente) limpa as colunas ocultas.
  useHiddenColumnsReset(
    columns,
    tab.hiddenColumns.length,
    useCallback(
      () => setQueryHiddenColumns(tab.id, []),
      [tab.id, setQueryHiddenColumns],
    ),
  );

  return (
    <ResizablePanelGroup direction="vertical" className="h-full">
      <ResizablePanel defaultSize={40} minSize={20}>
        <div className="flex flex-col h-full rounded-xl overflow-hidden">
          <QueryEditor
            className="h-full"
            height="100%"
            value={tab.content}
            dialect={supportsSql ? 'postgres' : 'mongo'}
            sqlCompletionSource={supportsSql ? sqlCompletion.source : undefined}
            sqlExtraExtensions={
              supportsSql ? sqlCompletion.prefetch : undefined
            }
            placeholderText={placeholder}
            onChange={setContent}
            onRun={executeRun}
            runMode="selection-or-all"
          />
        </div>
      </ResizablePanel>

      <ResizableHandle className="bg-transparent my-1 cursor-row-resize!" />

      <ResizablePanel defaultSize={60} minSize={20}>
        <div className="flex flex-col h-full gap-2">
          <div className="flex items-center gap-1.5">
            <ColumnSelector
              columns={columns}
              hiddenColumns={hiddenColumns}
              onChange={setHiddenColumns}
            />
          </div>

          <ResultsTable
            className="flex-1 min-h-0"
            columns={columns}
            rows={result?.rows ?? []}
            editableInfo={result?.editableInfo}
            hiddenColumns={hiddenColumns}
            isLoading={isRunning}
            isLoadingMore={isLoadingMore}
            hasMore={result?.hasMore ?? false}
            executionTimeMs={result?.executionTimeMs}
            totalCount={result?.totalCount}
            rowCount={result?.rowCount}
            emptyMessage={
              result
                ? 'A query não retornou resultados'
                : 'Execute uma query para ver resultados'
            }
            onSort={() => {}}
            onLoadMore={loadMore}
            onSave={save}
          />
        </div>
      </ResizablePanel>
    </ResizablePanelGroup>
  );
});

QueryEditorPanel.displayName = 'QueryEditorPanel';
