import { useMemo, useState } from 'react';
import type { CatalogNodeKind } from '@/api/types/catalog.types';
import type {
  RelationKind,
  StructureChange,
} from '@/api/types/structure.types';
import { useDbCapabilities } from '@/queries/use-capabilities';
import { useTabsStore } from '@/stores/tabs-store';
import type { DialogOf } from './structure-editor.types';
import { useStructureChangeDialog } from './use-structure-change-dialog';
import { tableTabId } from './use-rename-table-dialog';

type DropDialog = DialogOf<'dropTable'> | DialogOf<'dropColumn'>;

const RELATION_KIND: Partial<Record<CatalogNodeKind, RelationKind>> = {
  view: 'view',
  materializedView: 'materializedView',
  foreign: 'foreign',
};

/** O que o diálogo diz que vai remover */
function subjectOf(dialog: DropDialog): string {
  if (dialog.kind === 'dropColumn') return 'a coluna';
  switch (dialog.relationKind) {
    case 'view':
      return 'a view';
    case 'materializedView':
      return 'a view materializada';
    case 'foreign':
      return 'a tabela externa';
    default:
      return 'a tabela';
  }
}

/** Excluir tabela, view ou coluna — no Postgres, com CASCADE opcional */
export function useDropDialog(dialog: DropDialog) {
  const { target } = dialog;
  const [cascade, setCascade] = useState(false);
  const isColumn = dialog.kind === 'dropColumn';
  const name = isColumn ? dialog.column : dialog.table;

  const change = useMemo<StructureChange>(
    () =>
      dialog.kind === 'dropColumn'
        ? {
            type: 'dropColumn',
            schema: target.schema,
            table: dialog.table,
            column: dialog.column,
            cascade,
          }
        : {
            type: 'dropTable',
            schema: target.schema,
            table: dialog.table,
            kind:
              (dialog.relationKind && RELATION_KIND[dialog.relationKind]) ??
              'table',
            cascade,
          },
    [dialog, cascade, target.schema],
  );

  const subject = subjectOf(dialog);
  const capsByType = useDbCapabilities();
  const dialogState = useStructureChangeDialog(target, change, {
    success: `${name} foi excluída.`,
    // A aba da tabela que deixou de existir fecha
    onApplied: () => {
      if (dialog.kind === 'dropTable') {
        useTabsStore.getState().closeTab(tableTabId(dialog));
      }
    },
  });

  return {
    name,
    subject,
    isColumn,
    supportsCascade: !!capsByType[target.dbType]?.supportsDropCascade,
    cascade,
    setCascade,
    ...dialogState,
  };
}
