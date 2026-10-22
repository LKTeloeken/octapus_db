import { useMemo, useRef, useState } from 'react';
import type {
  ColumnDefinition,
  StructureChange,
} from '@/api/types/structure.types';
import { encodeNodeId } from '@/lib/node-ref';
import { useTreeStore } from '@/stores/tree-store';
import type { ColumnDraft, DialogOf } from './structure-editor.types';
import { defaultKeyColumn, emptyColumn } from './type-suggestions';
import { useStructureChangeDialog } from './use-structure-change-dialog';

const isComplete = (column: ColumnDefinition) =>
  !!column.name.trim() && !!column.dataType.trim();

/** Criar tabela: nome e colunas (começa com a chave `id`) */
export function useTableFormDialog({ target }: DialogOf<'createTable'>) {
  const nextKey = useRef(1);
  const [name, setName] = useState('');
  const [columns, setColumns] = useState<ColumnDraft[]>(() => [
    { key: 0, ...defaultKeyColumn(target.dbType) },
  ]);

  const change = useMemo<StructureChange | null>(() => {
    if (!name.trim() || columns.length === 0 || !columns.every(isComplete)) {
      return null;
    }
    return {
      type: 'createTable',
      schema: target.schema,
      table: name.trim(),
      columns: columns.map(({ key: _key, ...column }) => column),
    };
  }, [name, columns, target.schema]);

  const dialog = useStructureChangeDialog(target, change, {
    success: `Tabela ${name.trim()} criada.`,
    // A tabela nova aparece na pasta Tabelas do schema (ou do database, no
    // SQLite), aberta
    onApplied: () => {
      const ref = {
        serverId: target.serverId,
        database: target.database,
        schema: target.schema ?? undefined,
      };
      const { expandNode } = useTreeStore.getState();
      expandNode(encodeNodeId(ref));
      expandNode(encodeNodeId({ ...ref, folder: 'tables' }));
    },
  });

  const updateColumn = (key: number, patch: Partial<ColumnDefinition>) =>
    setColumns(previous =>
      previous.map(column =>
        column.key === key ? { ...column, ...patch } : column,
      ),
    );

  const addColumn = () =>
    setColumns(previous => [
      ...previous,
      { key: nextKey.current++, ...emptyColumn() },
    ]);

  const removeColumn = (key: number) =>
    setColumns(previous => previous.filter(column => column.key !== key));

  return {
    name,
    setName,
    columns,
    updateColumn,
    addColumn,
    removeColumn,
    ...dialog,
  };
}
