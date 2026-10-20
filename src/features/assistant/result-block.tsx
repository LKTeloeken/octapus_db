import { ResultsTable } from '@/components/results-table/results-table';
import type { QueryResult } from '@/api/types/query.types';

interface ResultBlockProps {
  attemptId: string;
  result: QueryResult;
  loadingMore: boolean;
  onLoadMore: () => void;
}

const noop = () => {};

/** Header (40) + status bar (32) + linhas de 28, até 288 px */
const gridHeight = (rows: number) => Math.min(288, 40 + 32 + 2 + rows * 28);

/**
 * O resultado na mesma grade do editor, paginado (C-09) e **sem**
 * `editableInfo` (RN-16). Zero linhas é dito com todas as letras (RN-21):
 * nada de grade vazia que pareça resposta.
 */
export function ResultBlock({
  attemptId,
  result,
  loadingMore,
  onLoadMore,
}: ResultBlockProps) {
  if (result.rows.length === 0) {
    return (
      <p className="rounded-md border border-line-subtle bg-surface-2 px-3 py-2 text-small text-fg-muted">
        A consulta rodou e não retornou linhas.
      </p>
    );
  }

  return (
    <div
      className="overflow-hidden rounded-md border border-line"
      style={{ height: gridHeight(result.rows.length) }}
    >
      <ResultsTable
        className="h-full"
        columns={result.columns}
        rows={result.rows}
        editableInfo={null}
        hasMore={result.hasMore}
        isLoadingMore={loadingMore}
        executionTimeMs={result.executionTimeMs}
        totalCount={result.totalCount}
        rowCount={result.rows.length}
        exportFileName="assistente"
        searchScopeId={`assistant:${attemptId}`}
        keyboardScope
        onSort={noop}
        onLoadMore={onLoadMore}
        onSave={noop}
      />
    </div>
  );
}
