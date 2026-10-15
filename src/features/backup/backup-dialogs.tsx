import { memo } from 'react';
import { useBackupDialogStore } from '@/stores/backup-dialog-store';
import { BackupDialog } from './backup-dialog';
import { RestoreDialog } from './restore-dialog';

/**
 * Os diálogos de backup/restauração abertos pela árvore. Cada abertura é um
 * formulário novo (a key zera o estado); o que fechou segue montado durante a
 * animação de saída.
 */
export const BackupDialogs = memo(() => {
  const dialog = useBackupDialogStore(state => state.dialog);
  const open = useBackupDialogStore(state => state.isOpen);
  const key = useBackupDialogStore(state => state.openCount);
  if (!dialog) return null;

  switch (dialog.kind) {
    case 'backup':
      return <BackupDialog key={key} open={open} dialog={dialog} />;
    case 'restore':
      return <RestoreDialog key={key} open={open} dialog={dialog} />;
  }
});

BackupDialogs.displayName = 'BackupDialogs';
