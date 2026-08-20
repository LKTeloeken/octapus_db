import { useCallback, useEffect, useMemo, useState } from 'react';
import toast from 'react-hot-toast';
import type { SortSpec } from '@/api/types/browse.types';
import { useHiddenColumnsReset } from '@/components/column-selector/use-hidden-columns-reset';
import type { SaveRowChanges } from '@/components/results-table/results-table.types';
import {
  useApplyRowEdits,
  useDeleteRows,
  useInsertRows,
} from '@/queries/use-apply-row-edits';
import { useCapabilities } from '@/queries/use-capabilities';
import { useTableData } from '@/queries/use-table-data';
import { useTabsStore, type BrowseTab } from '@/stores/tabs-store';

export const useTableBrowser = (tab: BrowseTab) => {
  const setBrowseSort = useTabsStore(state => state.setBrowseSort);
  const setBrowseWhere = useTabsStore(state => state.setBrowseWhere);
  const setBrowseHiddenColumns = useTabsStore(
    state => state.setBrowseHiddenColumns,
  );
  const applyEditsMutation = useApplyRowEdits();
  const insertRowsMutation = useInsertRows();
  const deleteRowsMutation = useDeleteRows();
  const { data: capabilities } = useCapabilities(tab.serverId);
  const supportsSql = capabilities?.supportsSql === true;

  const [draftWhere, setDraftWhere] = useState(tab.whereExpr);

  useEffect(() => {
    setDraftWhere(tab.whereExpr);
  }, [tab.id, tab.whereExpr]);

  const data = useTableData({
    serverId: tab.serverId,
    database: tab.database,
    schema: tab.schema,
    table: tab.table,
    whereExpr: tab.whereExpr,
    sort: tab.sort,
  });

  // The sort the backend is effectively applying: an explicit single-column
  // choice, or the PK-desc default (derived so the header can show it).
  const activeSort: SortSpec | null =
    tab.sort[0] ??
    (data.editableInfo?.primaryKeyColumns[0]
      ? { column: data.editableInfo.primaryKeyColumns[0], direction: 'desc' }
      : null);

  // Header click: sort by a single column. Re-clicking the active column flips
  // its direction; a new column starts ascending.
  const setSort = useCallback(
    (column: string) => {
      // Same column → flip direction; new column → start ascending.
      const direction: SortSpec['direction'] =
        activeSort?.column === column && activeSort.direction === 'asc'
          ? 'desc'
          : 'asc';

      setBrowseSort(tab.id, [{ column, direction }]);
    },
    [activeSort, tab.id, setBrowseSort],
  );

  const applyWhere = useCallback(() => {
    const next = draftWhere.trim();
    if (next !== tab.whereExpr) {
      setBrowseWhere(tab.id, next);
    } else {
      void data.refetch();
    }
  }, [draftWhere, tab.id, tab.whereExpr, setBrowseWhere, data.refetch]);

  const resetWhere = useCallback(() => {
    setDraftWhere(tab.whereExpr);
  }, [tab.whereExpr]);

  const hiddenColumns = useMemo(
    () => new Set(tab.hiddenColumns),
    [tab.hiddenColumns],
  );

  const setHiddenColumns = useCallback(
    (cols: string[]) => setBrowseHiddenColumns(tab.id, cols),
    [tab.id, setBrowseHiddenColumns],
  );

  // Uma nova query (conjunto de colunas diferente) limpa as colunas ocultas.
  const resetHiddenColumns = useCallback(
    () => setBrowseHiddenColumns(tab.id, []),
    [tab.id, setBrowseHiddenColumns],
  );
  useHiddenColumnsReset(
    data.columns,
    tab.hiddenColumns.length,
    resetHiddenColumns,
  );

  // Cmd/Ctrl+R applies the draft WHERE (if dirty) and reloads table data.
  useEffect(() => {
    const handler = (event: KeyboardEvent) => {
      if ((event.metaKey || event.ctrlKey) && event.key.toLowerCase() === 'r') {
        event.preventDefault();
        applyWhere();
      }
    };

    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [applyWhere]);

  // Persist deletes, inserts and edits together. Each mutation invalidates the
  // browse query, so the table only reflects changes once the backend confirms.
  const save = useCallback(
    async ({ edits, inserts, deletes }: SaveRowChanges) => {
      const editable = data.editableInfo;
      if (!editable) return;

      const base = { serverId: tab.serverId, database: tab.database, editable };

      try {
        let affected = 0;
        if (deletes.length > 0) {
          const r = await deleteRowsMutation.mutateAsync({
            ...base,
            pkValues: deletes,
          });
          affected += r.affectedRows;
        }
        if (inserts.length > 0) {
          const r = await insertRowsMutation.mutateAsync({
            ...base,
            rows: inserts,
          });
          affected += r.affectedRows;
        }
        if (edits.length > 0) {
          const r = await applyEditsMutation.mutateAsync({ ...base, edits });
          affected += r.affectedRows;
        }
        toast.success(`${affected} linhas alteradas`);
      } catch (error) {
        toast.error(error instanceof Error ? error.message : String(error));
        throw error; // let ResultsTable keep the pending changes for retry
      }
    },
    [
      data.editableInfo,
      tab.serverId,
      tab.database,
      applyEditsMutation,
      insertRowsMutation,
      deleteRowsMutation,
    ],
  );

  return {
    ...data,
    activeSort,
    hiddenColumns,
    draftWhere,
    setDraftWhere,
    applyWhere,
    resetWhere,
    supportsSql,
    setSort,
    setHiddenColumns,
    save,
  };
};
