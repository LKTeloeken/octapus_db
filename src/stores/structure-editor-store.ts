import { create } from 'zustand';
import type { CatalogNodeKind } from '@/api/types/catalog.types';
import type { DatabaseType } from '@/api/types/server.types';
import type { ColumnInfo } from '@/api/types/structure.types';

/** Onde a mudança acontece. `schema` null = banco sem schema (SQLite) */
export interface StructureTarget {
  serverId: number;
  database: string;
  dbType: DatabaseType;
  schema: string | null;
}

/**
 * Os diálogos de estrutura abertos pelo menu da árvore: criar, renomear e
 * excluir tabelas e colunas, e comparar dois schemas.
 */
export type StructureDialog =
  | { kind: 'createTable'; target: StructureTarget }
  | { kind: 'renameTable'; target: StructureTarget; table: string }
  | {
      kind: 'dropTable';
      target: StructureTarget;
      table: string;
      relationKind: CatalogNodeKind | undefined;
    }
  | { kind: 'addColumn'; target: StructureTarget; table: string }
  | {
      kind: 'editColumn';
      target: StructureTarget;
      table: string;
      column: ColumnInfo;
    }
  | {
      kind: 'dropColumn';
      target: StructureTarget;
      table: string;
      column: string;
    }
  | {
      kind: 'compareSchemas';
      serverId: number;
      database: string;
      /** Schema base já escolhido (aberto pelo menu de um schema) */
      base: string | null;
    };

interface StructureEditorState {
  dialog: StructureDialog | null;
  isOpen: boolean;
  /** Conta as aberturas: reabrir o mesmo diálogo começa um formulário novo */
  openCount: number;
  open: (dialog: StructureDialog) => void;
  close: () => void;
}

/**
 * O diálogo fica guardado depois de fechar: ele ainda aparece durante a
 * animação de saída, e limpar aqui trocaria o conteúdo no meio do fade.
 */
export const useStructureEditorStore = create<StructureEditorState>(set => ({
  dialog: null,
  isOpen: false,
  openCount: 0,
  open: dialog =>
    set(state => ({ dialog, isOpen: true, openCount: state.openCount + 1 })),
  close: () => set({ isOpen: false }),
}));
