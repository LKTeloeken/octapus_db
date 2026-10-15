import type { ColumnDefinition } from '@/api/types/structure.types';
import type { StructureDialog } from '@/stores/structure-editor-store';

/** Um diálogo específico do editor (o `kind` já resolvido) */
export type DialogOf<K extends StructureDialog['kind']> = Extract<
  StructureDialog,
  { kind: K }
>;

/** Coluna em edição na criação de tabela; `key` identifica a linha na lista */
export interface ColumnDraft extends ColumnDefinition {
  key: number;
}

export interface SqlPreviewProps {
  statements: string[] | undefined;
  error: string | null;
  isLoading: boolean;
  /** Texto quando não há o que rodar (nada mudou) */
  emptyLabel?: string;
}
