import { FolderAddIcon } from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Label } from '@/components/ui/label';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectSeparator,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { NO_GROUP, type GroupDialogProps } from './server-groups.types';
import { useMoveServerDialog } from './use-move-server-dialog';

/** Escolhe o grupo de um servidor (ou nenhum) */
export function MoveServerDialog({ open, dialog }: GroupDialogProps<'move'>) {
  const {
    groups,
    value,
    setValue,
    canMove,
    isMoving,
    submit,
    createGroup,
    close,
  } = useMoveServerDialog(dialog);

  return (
    <Dialog open={open} onOpenChange={next => !next && close()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>Mover {dialog.server.name}</DialogTitle>
          <DialogDescription>
            Só muda onde o servidor aparece na sidebar — a conexão não é
            afetada.
          </DialogDescription>
        </DialogHeader>

        <div className="flex flex-col gap-1.5">
          <Label htmlFor="move-server-group">Grupo</Label>
          <Select value={value} onValueChange={setValue}>
            <SelectTrigger id="move-server-group" className="w-full">
              <SelectValue />
            </SelectTrigger>
            <SelectContent>
              <SelectItem value={NO_GROUP}>Sem grupo</SelectItem>
              {groups.length > 0 && <SelectSeparator />}
              {groups.map(group => (
                <SelectItem key={group.id} value={String(group.id)}>
                  {group.name}
                </SelectItem>
              ))}
            </SelectContent>
          </Select>
        </div>

        <DialogFooter>
          <Button
            variant="ghost"
            size="sm"
            className="mr-auto"
            onClick={createGroup}
          >
            <HugeiconsIcon icon={FolderAddIcon} />
            Novo grupo
          </Button>
          <Button variant="ghost" size="sm" onClick={close}>
            Cancelar
          </Button>
          <Button size="sm" disabled={!canMove} onClick={() => void submit()}>
            {isMoving ? 'Movendo…' : 'Mover'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
