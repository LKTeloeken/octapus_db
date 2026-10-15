import type { Server } from '@/api/types/server.types';

export interface ServerFormProps {
  open: boolean;
  onClose: () => void;
  /** Non-null → edit mode */
  server: Server | null;
  /** Grupo já escolhido ao criar (menu do grupo); ignorado na edição */
  defaultGroupId?: number | null;
}
