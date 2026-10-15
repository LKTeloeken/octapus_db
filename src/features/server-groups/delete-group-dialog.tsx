import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { Button } from '@/components/ui/button';
import type { GroupDialogProps } from './server-groups.types';
import { useDeleteGroupDialog } from './use-delete-group-dialog';

/** Confirma a exclusão de um grupo — os servidores não são excluídos */
export function DeleteGroupDialog({
  open,
  dialog,
}: GroupDialogProps<'delete'>) {
  const { isDeleting, submit, close } = useDeleteGroupDialog(dialog);
  const { group, serverCount } = dialog;

  return (
    <AlertDialog open={open} onOpenChange={next => !next && close()}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>Excluir o grupo {group.name}?</AlertDialogTitle>
          <AlertDialogDescription>
            {serverCount === 0
              ? 'O grupo está vazio.'
              : serverCount === 1
                ? 'O servidor dele não é excluído: fica fora de grupo.'
                : `Os ${serverCount} servidores dele não são excluídos: ficam fora de grupo.`}
          </AlertDialogDescription>
        </AlertDialogHeader>

        <AlertDialogFooter>
          <Button variant="ghost" size="sm" onClick={close}>
            Cancelar
          </Button>
          <Button
            variant="destructive"
            size="sm"
            disabled={isDeleting}
            onClick={() => void submit()}
          >
            {isDeleting ? 'Excluindo…' : 'Excluir grupo'}
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
