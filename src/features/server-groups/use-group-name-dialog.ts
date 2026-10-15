import { useState } from 'react';
import toast from 'react-hot-toast';
import {
  useCreateServerGroup,
  useRenameServerGroup,
  useSetServerGroup,
} from '@/queries/use-server-groups';
import { useServerGroupDialogStore } from '@/stores/server-group-dialog-store';
import type { GroupDialogOf } from './server-groups.types';

/** Criar um grupo (e, vindo de "Mover para grupo", já mover o servidor) ou renomear */
export function useGroupNameDialog(dialog: GroupDialogOf<'create' | 'rename'>) {
  const close = useServerGroupDialogStore(state => state.close);
  const isRename = dialog.kind === 'rename';
  const [name, setName] = useState(isRename ? dialog.group.name : '');

  const createGroup = useCreateServerGroup();
  const renameGroup = useRenameServerGroup();
  const setServerGroup = useSetServerGroup();

  const isSaving = createGroup.isPending || renameGroup.isPending;
  const trimmed = name.trim();
  const canSave =
    !!trimmed && !isSaving && !(isRename && trimmed === dialog.group.name);

  const submit = async () => {
    if (!canSave) return;
    try {
      if (isRename) {
        await renameGroup.mutateAsync({ id: dialog.group.id, name: trimmed });
        toast.success('Grupo renomeado.');
      } else {
        const group = await createGroup.mutateAsync(trimmed);
        if (dialog.moveServerId != null) {
          await setServerGroup.mutateAsync({
            id: dialog.moveServerId,
            groupId: group.id,
          });
          toast.success(`Servidor movido para ${group.name}.`);
        } else {
          toast.success('Grupo criado.');
        }
      }
      close();
    } catch (error) {
      toast.error(error instanceof Error ? error.message : String(error));
    }
  };

  return { isRename, name, setName, canSave, isSaving, submit, close };
}
