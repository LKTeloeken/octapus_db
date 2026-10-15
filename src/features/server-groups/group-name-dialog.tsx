import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Input } from '@/components/ui/input/input';
import type { GroupDialogProps } from './server-groups.types';
import { useGroupNameDialog } from './use-group-name-dialog';

/** Nome do grupo, ao criar ou renomear */
export function GroupNameDialog({
  open,
  dialog,
}: GroupDialogProps<'create' | 'rename'>) {
  const { isRename, name, setName, canSave, isSaving, submit, close } =
    useGroupNameDialog(dialog);

  return (
    <Dialog open={open} onOpenChange={next => !next && close()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>
            {isRename ? 'Renomear grupo' : 'Novo grupo'}
          </DialogTitle>
          <DialogDescription>
            {isRename
              ? 'Os servidores continuam no grupo.'
              : dialog.kind === 'create' && dialog.moveServerId != null
                ? 'O servidor vai para o grupo novo.'
                : 'Grupos organizam os servidores na sidebar.'}
          </DialogDescription>
        </DialogHeader>

        <form
          onSubmit={event => {
            event.preventDefault();
            void submit();
          }}
        >
          <Input
            label="Nome"
            placeholder="Produção, Clientes, Local…"
            autoFocus
            value={name}
            onChange={event => setName(event.target.value)}
            onFocus={event => event.target.select()}
          />
        </form>

        <DialogFooter>
          <Button variant="ghost" size="sm" onClick={close}>
            Cancelar
          </Button>
          <Button size="sm" disabled={!canSave} onClick={() => void submit()}>
            {isRename
              ? isSaving
                ? 'Renomeando…'
                : 'Renomear'
              : isSaving
                ? 'Criando…'
                : 'Criar'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
