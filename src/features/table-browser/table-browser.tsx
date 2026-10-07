import { HugeiconsIcon } from '@hugeicons/react';
import {
  Alert02Icon,
  ArrowRight01Icon,
  PanelRightIcon,
  RefreshIcon,
  ServerStack01Icon,
} from '@hugeicons/core-free-icons';
import { memo } from 'react';
import { ColumnSelector } from '@/components/column-selector/column-selector';
import { ResultsTable } from '@/components/results-table/results-table';
import { Button } from '@/components/ui/button';
import { ContentState } from '@/components/ui/content-state';
import { Tooltip } from '@/components/ui/tooltip/tooltip';
import { DB_TYPE_TEXT_COLOR } from '@/lib/db-defaults';
import { shortcut } from '@/lib/platform';
import { cn } from '@/lib/utils';
import { FilterInput } from './filter-input/filter-input';
import { useTableBrowser } from './use-table-browser';
import type { TableBrowserProps } from './table-browser.types';

const Crumb = () => (
  <HugeiconsIcon
    icon={ArrowRight01Icon}
    className="size-3 shrink-0 text-fg-disabled"
  />
);

/**
 * Browse view of a table/collection/key-group. Sort and pagination happen
 * server-side via fetch_table_data. On Postgres, an optional WHERE expression
 * is sent as `whereExpr` — the backend still owns SELECT/ORDER/LIMIT.
 */
export const TableBrowser = memo(({ tab }: TableBrowserProps) => {
  const {
    columns,
    rows,
    editableInfo,
    totalCount,
    rowCount,
    executionTimeMs,
    hasMore,
    isLoading,
    isLoadingMore,
    error,
    activeSort,
    hiddenColumns,
    draftWhere,
    setDraftWhere,
    applyWhere,
    resetWhere,
    supportsSql,
    whereCompletionSource,
    whereCompletionExtensions,
    serverName,
    dbType,
    fetchNextPage,
    setSort,
    setHiddenColumns,
    save,
    fetchAllRows,
    isValuePanelOpen,
    toggleValuePanel,
    closeValuePanel,
  } = useTableBrowser(tab);

  return (
    <div className="flex h-full flex-col overflow-hidden rounded-lg border border-line bg-surface-1">
      <div className="flex h-11 shrink-0 items-center gap-2 border-b border-line-subtle pr-2 pl-3">
        <nav
          aria-label="Local"
          className="flex min-w-0 shrink-0 items-center gap-1.5 text-body text-fg-subtle"
        >
          <HugeiconsIcon
            icon={ServerStack01Icon}
            className={cn(
              'size-[15px] shrink-0',
              dbType ? DB_TYPE_TEXT_COLOR[dbType] : 'text-fg-subtle',
            )}
          />
          <span className="max-w-40 truncate">{serverName}</span>
          <Crumb />
          <span className="max-w-32 truncate">
            {tab.schema ?? tab.database}
          </span>
          <Crumb />
          <span className="max-w-48 truncate font-medium text-fg">
            {tab.table}
          </span>
        </nav>

        <span aria-hidden className="mx-1 h-4 w-px shrink-0 bg-line" />

        {supportsSql ? (
          <FilterInput
            value={draftWhere}
            onChange={setDraftWhere}
            onApply={applyWhere}
            onReset={resetWhere}
            completionSource={whereCompletionSource}
            completionExtensions={whereCompletionExtensions}
          />
        ) : (
          // Sem WHERE (Mongo/Redis) os botões ainda ficam à direita.
          <span className="flex-1" />
        )}

        <ColumnSelector
          columns={columns}
          hiddenColumns={hiddenColumns}
          onChange={setHiddenColumns}
        />

        <Tooltip
          content={`Painel de valor (${shortcut('I')})`}
          position="bottom"
        >
          <Button
            variant="ghost"
            size="sm"
            aria-pressed={isValuePanelOpen}
            onClick={toggleValuePanel}
          >
            <HugeiconsIcon icon={PanelRightIcon} />
            Valor
          </Button>
        </Tooltip>

        <Tooltip content={`Recarregar (${shortcut('R')})`} position="bottom">
          <Button
            variant="ghost"
            size="icon"
            aria-label="Recarregar"
            onClick={() => applyWhere()}
          >
            <HugeiconsIcon icon={RefreshIcon} />
          </Button>
        </Tooltip>
      </div>

      {error ? (
        <ContentState
          className="flex-1"
          tone="danger"
          icon={Alert02Icon}
          title="A consulta falhou"
          description={
            <code className="inline-block max-w-md rounded-sm bg-danger-soft px-2.5 py-1.5 font-mono text-small break-words text-danger">
              {error.message}
            </code>
          }
          action={
            <Button variant="outline" size="sm" onClick={() => applyWhere()}>
              <HugeiconsIcon icon={RefreshIcon} />
              Tentar de novo
            </Button>
          }
        />
      ) : (
        <ResultsTable
          className="flex-1 min-h-0"
          columns={columns}
          rows={rows}
          editableInfo={editableInfo}
          hiddenColumns={hiddenColumns}
          activeSort={activeSort}
          emptyMessage={
            tab.whereExpr.trim().length > 0
              ? 'Nenhum registro corresponde aos filtros'
              : 'Sem resultados na tabela'
          }
          isLoading={isLoading}
          isLoadingMore={isLoadingMore}
          hasMore={hasMore}
          executionTimeMs={executionTimeMs}
          totalCount={totalCount}
          rowCount={rowCount}
          exportFileName={tab.table}
          onSort={setSort}
          onLoadMore={fetchNextPage}
          onSave={save}
          onFetchAllRows={fetchAllRows}
          showValuePanel={isValuePanelOpen}
          onCloseValuePanel={closeValuePanel}
        />
      )}
    </div>
  );
});

TableBrowser.displayName = 'TableBrowser';
