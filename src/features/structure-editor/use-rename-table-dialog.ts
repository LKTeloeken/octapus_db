import { useMemo, useState } from 'react';
import type { StructureChange } from '@/api/types/structure.types';
import { encodeNodeId } from '@/lib/node-ref';
import { useTabsStore } from '@/stores/tabs-store';
import type { DialogOf } from './structure-editor.types';
import { useStructureChangeDialog } from './use-structure-change-dialog';

/** A aba de browse tem o mesmo id do nó da tabela na árvore */
export function tableTabId(dialog: {
  target: DialogOf<'renameTable'>['target'];
  table: string;
}) {
  const { target, table } = dialog;
  return encodeNodeId({
    serverId: target.serverId,
    database: target.database,
    schema: target.schema ?? undefined,
    table,
  });
}

/** Renomear tabela: a aba aberta com o nome antigo fecha */
export function useRenameTableDialog(dialog: DialogOf<'renameTable'>) {
  const { target, table } = dialog;
  const [name, setName] = useState(table);

  const change = useMemo<StructureChange | null>(() => {
    const newName = name.trim();
    if (!newName || newName === table) return null;
    return { type: 'renameTable', schema: target.schema, table, newName };
  }, [name, table, target.schema]);

  const dialogState = useStructureChangeDialog(target, change, {
    success: `Tabela renomeada para ${name.trim()}.`,
    onApplied: () => useTabsStore.getState().closeTab(tableTabId(dialog)),
  });

  return { name, setName, ...dialogState };
}
