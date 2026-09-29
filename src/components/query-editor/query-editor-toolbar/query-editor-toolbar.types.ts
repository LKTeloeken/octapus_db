import type { DatabaseType } from '@/api/types/server.types';

export interface QueryEditorToolbarProps {
  onRun: () => void;
  isLoading?: boolean;
  disabled?: boolean;
  /** Servidor e banco da aba, mostrados ao lado do Executar */
  serverName: string;
  database: string;
  dbType?: DatabaseType;
}
