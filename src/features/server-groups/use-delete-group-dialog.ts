import toast from 'react-hot-toast';
import { useDeleteServerGroup } from '@/queries/use-server-groups';
import { useServerGroupDialogStore } from '@/stores/server-group-dialog-store';
import type { GroupDialogOf } from './server-groups.types';

/** Excluir o grupo: os servidores dele ficam fora de grupo */
export function useDeleteGroupDialog(dialog: GroupDialogOf<'delete'>) {
  const close = useServerGroupDialogStore(state => state.close);
  const deleteGroup = useDeleteServerGroup();

  const submit = async () => {
    try {
      await deleteGroup.mutateAsync(dialog.group.id);
      toast.success('Grupo excluído.');
      close();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    }
  };

  return { isDeleting: deleteGroup.isPending, submit, close };
}
