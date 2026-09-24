import type { QueryColumnInfo } from '@/api/types/query.types';
import type { UpdateCellFn } from '../results-table.types';

/** A célula mostrada no painel — a do cursor de teclado da grade. */
export interface ValuePanelTarget {
  rowIndex: number;
  column: QueryColumnInfo;
  /** Valor da linha como veio do banco: o `originalValue` do `updateCell` */
  originalValue: string | null;
  /** Valor exibido, já com a edição pendente (se houver) */
  value: string | null;
  isEditable: boolean;
  isModified: boolean;
}

export interface ResultsTableValuePanelProps {
  /** null quando nenhuma célula está sob o cursor */
  target: ValuePanelTarget | null;
  updateCell: UpdateCellFn;
  onClose?: () => void;
  /** Esc dentro do editor: devolve o foco para a grade */
  onEscape?: () => void;
}

/** Aviso mostrado no rodapé; `blocking` = a digitação não foi aplicada. */
export interface ValueIssue {
  message: string;
  blocking: boolean;
}
