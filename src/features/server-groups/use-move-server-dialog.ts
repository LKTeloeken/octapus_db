import { useState } from 'react';
import toast from 'react-hot-toast';
import {
  useServerGroups,
  useSetServerGroup,
} from '@/queries/use-server-groups';
import { useServerGroupDialogStore } from '@/stores/server-group-dialog-store';
import { NO_GROUP, type GroupDialogOf } from './server-groups.types';

/** Mover um servidor de grupo — inclusive de um grupo para outro */
export function useMoveServerDialog(dialog: GroupDialogOf<'move'>) {
  const { server } = dialog;
  const close = useServerGroupDialogStore(state => state.close);
  const openDialog = useServerGroupDialogStore(state => state.open);
  const { data: groups = [] } = useServerGroups();
  const setServerGroup = useSetServerGroup();

  const current = server.groupId == null ? NO_GROUP : String(server.groupId);
  const [value, setValue] = useState(current);

  const submit = async () => {
    const groupId = value === NO_GROUP ? null : Number(value);
    try {
      await setServerGroup.mutateAsync({ id: server.id, groupId });
      const target = groups.find(group => group.id === groupId);
      toast.success(
        target
          ? `Servidor movido para ${target.name}.`
          : 'Servidor fora de grupo.',
      );
      close();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    }
  };

  /** Troca este diálogo pelo de grupo novo, que já move o servidor ao criar */
  const createGroup = () =>
    openDialog({ kind: 'create', moveServerId: server.id });

  return {
    groups,
    value,
    setValue,
    canMove: value !== current && !setServerGroup.isPending,
    isMoving: setServerGroup.isPending,
    submit,
    createGroup,
    close,
  };
}
