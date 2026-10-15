import { memo } from 'react';
import { SchemaCompareDialog } from '@/features/schema-compare/schema-compare-dialog';
import { useStructureEditorStore } from '@/stores/structure-editor-store';
import { ColumnFormDialog } from './column-form-dialog';
import { DropDialog } from './drop-dialog';
import { RenameTableDialog } from './rename-table-dialog';
import { TableFormDialog } from './table-form-dialog';

/**
 * Os diálogos de estrutura abertos pelo menu da árvore. Um por vez; cada
 * abertura é um diálogo novo (a key zera o formulário), e o que fechou continua
 * montado durante a animação de saída.
 */
export const StructureEditor = memo(() => {
  const dialog = useStructureEditorStore(state => state.dialog);
  const open = useStructureEditorStore(state => state.isOpen);
  const key = useStructureEditorStore(state => state.openCount);
  if (!dialog) return null;

  switch (dialog.kind) {
    case 'createTable':
      return <TableFormDialog key={key} open={open} dialog={dialog} />;
    case 'addColumn':
    case 'editColumn':
      return <ColumnFormDialog key={key} open={open} dialog={dialog} />;
    case 'renameTable':
      return <RenameTableDialog key={key} open={open} dialog={dialog} />;
    case 'dropTable':
    case 'dropColumn':
      return <DropDialog key={key} open={open} dialog={dialog} />;
    case 'compareSchemas':
      return <SchemaCompareDialog key={key} open={open} dialog={dialog} />;
  }
});

StructureEditor.displayName = 'StructureEditor';
