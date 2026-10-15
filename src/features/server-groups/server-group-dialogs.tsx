import { memo } from 'react';
import { useServerGroupDialogStore } from '@/stores/server-group-dialog-store';
import { DeleteGroupDialog } from './delete-group-dialog';
import { GroupNameDialog } from './group-name-dialog';
import { MoveServerDialog } from './move-server-dialog';

/**
 * Os diálogos de grupo de servidores. Um por vez; cada abertura é um diálogo
 * novo (a key zera o formulário), e o que fechou continua montado durante a
 * animação de saída.
 */
export const ServerGroupDialogs = memo(() => {
  const dialog = useServerGroupDialogStore(state => state.dialog);
  const open = useServerGroupDialogStore(state => state.isOpen);
  const key = useServerGroupDialogStore(state => state.openCount);
  if (!dialog) return null;

  switch (dialog.kind) {
    case 'create':
    case 'rename':
      return <GroupNameDialog key={key} open={open} dialog={dialog} />;
    case 'delete':
      return <DeleteGroupDialog key={key} open={open} dialog={dialog} />;
    case 'move':
      return <MoveServerDialog key={key} open={open} dialog={dialog} />;
  }
});

ServerGroupDialogs.displayName = 'ServerGroupDialogs';
