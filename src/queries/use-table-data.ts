import { useInfiniteQuery } from '@tanstack/react-query';
import { useMemo } from 'react';
import { fetchTableData } from '@/api/browse';
import type { ColumnFilter, SortSpec } from '@/api/types/browse.types';
import type {
  EditableInfo,
  QueryColumnInfo,
} from '@/api/types/query.types';
import { queryKeys } from './keys';

export const TABLE_DATA_PAGE_SIZE = 500;

export interface UseTableDataParams {
  serverId: number;
  database: string;
  /** null for Mongo/Redis (hasSchemas=false) */
  schema: string | null;
  table: string;
  filters?: ColumnFilter[];
  sort?: SortSpec[];
  limit?: number;
  enabled?: boolean;
}

export interface UseTableDataResult {
  columns: QueryColumnInfo[];
  rows: (string | null)[][];
  editableInfo: EditableInfo | null;
  totalCount: number | null;
  rowCount: number;
  executionTimeMs: number | undefined;
  hasMore: boolean;
  isLoading: boolean;
  isFetching: boolean;
  isLoadingMore: boolean;
  error: Error | null;
  fetchNextPage: () => void;
  refetch: () => void;
}

/**
 * Server-side browse of a table/collection/key-group via fetch_table_data.
 * Changing filters/sort changes the query key → fresh first page; pagination
 * accumulates pages through useInfiniteQuery.
 */
export function useTableData(
  params: UseTableDataParams | null,
): UseTableDataResult {
  const limit = params?.limit ?? TABLE_DATA_PAGE_SIZE;
  const filters = params?.filters ?? [];
  const sort = params?.sort ?? [];

  const query = useInfiniteQuery({
    queryKey: queryKeys.tableData(
      params?.serverId ?? -1,
      params?.database ?? '',
      params?.schema ?? null,
      params?.table ?? '',
      filters,
      sort,
    ),
    queryFn: ({ pageParam }) =>
      fetchTableData(params!.serverId, params!.database, {
        schema: params!.schema,
        table: params!.table,
        filters,
        sort,
        limit,
        offset: pageParam,
        countTotal: pageParam === 0,
      }),
    initialPageParam: 0,
    getNextPageParam: (lastPage, allPages) =>
      lastPage.hasMore
        ? allPages.reduce((total, page) => total + page.rowCount, 0)
        : undefined,
    enabled: params != null && (params.enabled ?? true),
    staleTime: 0,
  });

  const pages = query.data?.pages;

  const rows = useMemo(
    () => pages?.flatMap(page => page.rows) ?? [],
    [pages],
  );

  const firstPage = pages?.[0];
  const lastPage = pages?.[pages.length - 1];

  return {
    columns: firstPage?.columns ?? [],
    rows,
    editableInfo: firstPage?.editableInfo ?? null,
    totalCount: firstPage?.totalCount ?? null,
    rowCount: rows.length,
    executionTimeMs: firstPage?.executionTimeMs,
    hasMore: lastPage?.hasMore ?? false,
    isLoading: query.isLoading,
    isFetching: query.isFetching,
    isLoadingMore: query.isFetchingNextPage,
    error: query.error,
    fetchNextPage: () => {
      if (query.hasNextPage && !query.isFetchingNextPage) {
        void query.fetchNextPage();
      }
    },
    refetch: () => void query.refetch(),
  };
}
