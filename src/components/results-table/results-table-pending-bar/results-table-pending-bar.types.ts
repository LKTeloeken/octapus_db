import type { SaveFn } from '../results-table.types';

export interface ResultsTablePendingBarProps {
  /** Células editadas (fora das linhas marcadas para exclusão) */
  changesCount: number;
  /** Linhas novas a inserir */
  addedCount: number;
  /** Linhas existentes a excluir */
  removedCount: number;
  onDiscard: () => void;
  onSave: SaveFn;
}
