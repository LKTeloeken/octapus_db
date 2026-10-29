import type { EditableInfo } from '@/api/types/query.types';
import type { DataTableRow } from '../results-table/results-table.types';

export type ExportFormat = 'csv' | 'json' | 'sql';

/** Coluna exibida na grade, com o índice dela na linha crua (as ocultas ficam de fora) */
export interface ExportColumn {
  name: string;
  typeName: string;
  index: number;
}

export interface ExportOptions {
  format: ExportFormat;
  /** CSV: separador entre campos */
  delimiter: string;
  /** CSV: primeira linha com os nomes das colunas */
  includeHeader: boolean;
  /** SQL: tabela alvo dos INSERTs, já qualificada e escapada */
  sqlTable: string;
  /**
   * SQL: colunas deixadas fora dos INSERTs para o banco aplicar o valor
   * padrão (DEFAULT, serial, identity) — vale em qualquer dialeto, ao
   * contrário da palavra `DEFAULT` no VALUES, que o SQLite não aceita
   */
  sqlDefaultColumns?: ReadonlySet<string>;
}

export interface ExportDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
  columns: ExportColumn[];
  /** Linhas já carregadas na grade */
  rows: DataTableRow[];
  /** Linhas selecionadas na grade; vazio esconde a opção de exportar só elas */
  selectedRows: DataTableRow[];
  /** true quando a grade tem só parte do resultado */
  hasMore: boolean;
  /** Total no banco, quando o backend contou */
  totalCount?: number | null;
  /** Nome base sugerido para o arquivo, sem extensão */
  fileName: string;
  /** Sem ela o formato SQL não é oferecido (não há tabela para o INSERT) */
  editableInfo?: EditableInfo | null;
  /** Colunas cujo valor o banco gera: o SQL pode deixá-las para ele */
  defaultColumns?: ReadonlySet<string>;
  /** Busca o resultado inteiro no banco; sem ela só dá para exportar o carregado */
  onFetchAllRows?: () => Promise<DataTableRow[]>;
}
