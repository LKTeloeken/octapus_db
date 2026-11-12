import type { DatabaseType } from '@/api/types/server.types';

export interface QueryEditorToolbarProps {
  onRun: () => void;
  isLoading?: boolean;
  /**
   * Cancela a execução em andamento. Com ele, o Executar vira Cancelar
   * enquanto a consulta roda; sem ele (banco sem cancelamento), só espera.
   */
  onCancel?: () => void;
  /** O pedido de cancelamento já foi: o botão espera a consulta parar */
  isCancelling?: boolean;
  disabled?: boolean;
  /** Servidor e banco da aba, mostrados ao lado do Executar */
  serverName: string;
  database: string;
  dbType?: DatabaseType;
}
