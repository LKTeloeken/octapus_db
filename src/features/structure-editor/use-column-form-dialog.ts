import { useMemo, useState } from 'react';
import type {
  ColumnDefinition,
  StructureChange,
} from '@/api/types/structure.types';
import type { DialogOf } from './structure-editor.types';
import { emptyColumn } from './type-suggestions';
import { useStructureChangeDialog } from './use-structure-change-dialog';

type ColumnDialog = DialogOf<'addColumn'> | DialogOf<'editColumn'>;

/** A coluna como estava, no formato do formulário */
function fromInfo(dialog: ColumnDialog): ColumnDefinition {
  if (dialog.kind === 'addColumn') return emptyColumn();
  const { column } = dialog;
  return {
    name: column.name,
    dataType: column.dataType,
    nullable: column.isNullable,
    defaultValue: column.defaultValue,
    primaryKey: column.isPrimaryKey,
  };
}

/** Nova coluna ou edição de uma existente (nome, tipo, nulo, default) */
export function useColumnFormDialog(dialog: ColumnDialog) {
  const { target, table } = dialog;
  const isEdit = dialog.kind === 'editColumn';
  const [original] = useState(() => fromInfo(dialog));
  const [column, setColumn] = useState(original);

  const change = useMemo<StructureChange | null>(() => {
    if (!column.name.trim() || !column.dataType.trim()) return null;
    return isEdit
      ? {
          type: 'alterColumn',
          schema: target.schema,
          table,
          original,
          updated: column,
        }
      : { type: 'addColumn', schema: target.schema, table, column };
  }, [column, original, isEdit, target.schema, table]);

  const dialogState = useStructureChangeDialog(target, change, {
    success: isEdit
      ? `Coluna ${column.name.trim()} alterada.`
      : `Coluna ${column.name.trim()} adicionada a ${table}.`,
  });

  const setField = <K extends keyof ColumnDefinition>(
    key: K,
    value: ColumnDefinition[K],
  ) => setColumn(previous => ({ ...previous, [key]: value }));

  // O SQLite não muda tipo, nulo nem default: a tabela é recriada
  const rebuilds =
    isEdit &&
    target.dbType === 'sqlite' &&
    (dialogState.preview.statements?.length ?? 0) > 1 &&
    (column.dataType.trim().toLowerCase() !==
      original.dataType.trim().toLowerCase() ||
      column.nullable !== original.nullable ||
      (column.defaultValue?.trim() || null) !==
        (original.defaultValue?.trim() || null));

  return { isEdit, column, setField, rebuilds, ...dialogState };
}
