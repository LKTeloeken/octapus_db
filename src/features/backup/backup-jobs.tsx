import { useBackupJobs } from './use-backup-jobs';

/**
 * Os backups em andamento, como cards no canto (toasts). Montado no shell: o
 * job segue depois que o diálogo fecha.
 */
export function BackupJobs() {
  useBackupJobs();
  return null;
}
