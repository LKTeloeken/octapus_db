import { memo, useCallback, useMemo, useRef } from 'react';
import { ColumnSelector } from '@/components/column-selector/column-selector';
import { useHiddenColumnsReset } from '@/components/column-selector/use-hidden-columns-reset';
import {
  QueryEditor,
  type QueryEditorHandle,
} from '@/components/query-editor/query-editor/query-editor';
import { QueryEditorToolbar } from '@/components/query-editor/query-editor-toolbar/query-editor-toolbar';
import { QueryMessagesLog } from '@/components/query-messages/query-messages-log';
import { ResultsTable } from '@/components/results-table/results-table';
import { Badge } from '@/components/ui/badge';
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from '@/components/ui/resizable';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import type { BottomTab } from '@/stores/query-results-store';
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
    serverName,
    dbType,
    sqlCompletion,
    setContent,
    executeRun,
    loadMore,
    save,
    fetchAllRows,
    log,
    unreadMessages,
    clearLog,
    bottomTab,
    setBottomTab,
  } = useQueryRunner(tab);

  // O Executar da toolbar pede ao editor o mesmo trecho do Mod-Enter.
  const editorRef = useRef<QueryEditorHandle>(null);
  const runFromToolbar = useCallback(() => editorRef.current?.run(), []);

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

  const handleBottomTabChange = useCallback(
    (value: string) => setBottomTab(value as BottomTab),
    [setBottomTab],
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
    <ResizablePanelGroup direction="vertical" className="h-full gap-1">
      <ResizablePanel
        defaultSize={40}
        minSize={20}
        className="overflow-hidden rounded-lg border border-line bg-surface-1"
      >
        <div className="flex h-full flex-col overflow-hidden">
          <QueryEditorToolbar
            onRun={runFromToolbar}
            isLoading={isRunning}
            serverName={serverName}
            database={tab.database}
            dbType={dbType}
          />
          <QueryEditor
            ref={editorRef}
            className="min-h-0 flex-1"
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

      <ResizableHandle withHandle className="bg-transparent cursor-row-resize!" />

      <ResizablePanel
        defaultSize={60}
        minSize={20}
        className="overflow-hidden rounded-lg border border-line bg-surface-1"
      >
        <Tabs
          value={bottomTab}
          onValueChange={handleBottomTabChange}
          className="flex flex-col h-full gap-0"
        >
          <div className="flex h-10 shrink-0 items-center justify-between gap-2 border-b border-line-subtle pr-1.5 pl-3.5">
            <TabsList>
              <TabsTrigger value="results">Resultados</TabsTrigger>
              <TabsTrigger value="messages">
                Mensagens
                {unreadMessages > 0 && (
                  <Badge className="h-[18px] px-1.5">
                    {unreadMessages}
                  </Badge>
                )}
              </TabsTrigger>
            </TabsList>

            {bottomTab === 'results' && (
              <ColumnSelector
                columns={columns}
                hiddenColumns={hiddenColumns}
                onChange={setHiddenColumns}
              />
            )}
          </div>

          {/*
            forceMount + hidden: o Radix desmontaria o conteúdo inativo, e a
            ResultsTable perderia scroll, seleção e as edições pendentes toda
            vez que o usuário desse uma olhada no log.
          */}
          <TabsContent
            value="results"
            forceMount
            className="flex flex-col min-h-0 data-[state=inactive]:hidden"
          >
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
              exportFileName={tab.title}
              onSort={() => {}}
              onLoadMore={loadMore}
              onSave={save}
              onFetchAllRows={fetchAllRows}
            />
          </TabsContent>

          <TabsContent
            value="messages"
            forceMount
            className="flex flex-col min-h-0 data-[state=inactive]:hidden"
          >
            <QueryMessagesLog
              className="flex-1 min-h-0"
              entries={log}
              onClear={clearLog}
            />
          </TabsContent>
        </Tabs>
      </ResizablePanel>
    </ResizablePanelGroup>
  );
});

QueryEditorPanel.displayName = 'QueryEditorPanel';
