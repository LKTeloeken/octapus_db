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
import { SqlPreview } from './sql-preview';
import type { DialogOf } from './structure-editor.types';
import { useRenameTableDialog } from './use-rename-table-dialog';

interface RenameTableDialogProps {
  open: boolean;
  dialog: DialogOf<'renameTable'>;
}

export function RenameTableDialog({ open, dialog }: RenameTableDialogProps) {
  const { name, setName, preview, canApply, isApplying, submit, close } =
    useRenameTableDialog(dialog);
  const { target, table } = dialog;

  return (
    <Dialog open={open} onOpenChange={next => !next && close()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>
            Renomear <span className="font-mono">{table}</span>
          </DialogTitle>
          <DialogDescription>
            Em{' '}
            <span className="font-mono text-fg">
              {target.schema ?? target.database}
            </span>
            . Queries e views que citam o nome antigo não são reescritas.
          </DialogDescription>
        </DialogHeader>

        <form
          className="flex flex-col gap-5"
          onSubmit={event => {
            event.preventDefault();
            void submit();
          }}
        >
          <Input
            label="Novo nome"
            autoFocus
            inputClassName="font-mono"
            value={name}
            onChange={event => setName(event.target.value)}
            onFocus={event => event.target.select()}
          />
          <SqlPreview {...preview} emptyLabel="Digite o novo nome." />
        </form>

        <DialogFooter>
          <Button variant="ghost" size="sm" onClick={close}>
            Cancelar
          </Button>
          <Button size="sm" disabled={!canApply} onClick={() => void submit()}>
            {isApplying ? 'Renomeando…' : 'Renomear'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
