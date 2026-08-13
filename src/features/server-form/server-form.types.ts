import type { Server } from '@/api/types/server.types';

export interface ServerFormProps {
  open: boolean;
  onClose: () => void;
  /** Non-null → edit mode */
  server: Server | null;
}
