import { useEffect } from 'react';
import { onBackupEvent } from '@/api/backup';
import { useBackupJobsStore } from '@/stores/backup-jobs-store';

/**
 * Liga o `backup-event` à lista de jobs. Montado uma vez, no shell do app: o
 * backup continua (e termina) com o diálogo já fechado.
 */
export function useBackupEvents() {
  useEffect(() => {
    let unlisten: (() => void) | null = null;
    let disposed = false;
    const apply = useBackupJobsStore.getState().apply;

    void onBackupEvent(apply).then(stop => {
      if (disposed) stop();
      else unlisten = stop;
    });

    return () => {
      disposed = true;
      unlisten?.();
    };
  }, []);
}
