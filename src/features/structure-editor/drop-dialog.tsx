import {
  AlertDialog,
  AlertDialogContent,
  AlertDialogDescription,
  AlertDialogFooter,
  AlertDialogHeader,
  AlertDialogTitle,
} from '@/components/ui/alert-dialog';
import { Button } from '@/components/ui/button';
import { Checkbox } from '@/components/ui/checkbox';
import { Label } from '@/components/ui/label';
import { SqlPreview } from './sql-preview';
import type { DialogOf } from './structure-editor.types';
import { useDropDialog } from './use-drop-dialog';

interface DropDialogProps {
  open: boolean;
  dialog: DialogOf<'dropTable'> | DialogOf<'dropColumn'>;
}

/** Confirma a exclusão de uma tabela, view ou coluna, mostrando o SQL */
export function DropDialog({ open, dialog }: DropDialogProps) {
  const {
    name,
    subject,
    isColumn,
    supportsCascade,
    cascade,
    setCascade,
    preview,
    canApply,
    isApplying,
    submit,
    close,
  } = useDropDialog(dialog);
  const { target } = dialog;
  const where = isColumn
    ? `${target.schema ? `${target.schema}.` : ''}${dialog.table}`
    : (target.schema ?? target.database);

  return (
    <AlertDialog open={open} onOpenChange={next => !next && close()}>
      <AlertDialogContent>
        <AlertDialogHeader>
          <AlertDialogTitle>
            Excluir {subject} <span className="font-mono">{name}</span>?
          </AlertDialogTitle>
          <AlertDialogDescription>
            Em <span className="font-mono text-fg">{where}</span>.{' '}
            {isColumn
              ? 'Os valores da coluna em todas as linhas são perdidos.'
              : 'Os dados são perdidos.'}{' '}
            Não dá para desfazer.
          </AlertDialogDescription>
        </AlertDialogHeader>

        {supportsCascade && (
          <div className="flex items-center gap-2">
            <Checkbox
              id="drop-cascade"
              checked={cascade}
              onCheckedChange={checked => setCascade(checked === true)}
            />
            <Label htmlFor="drop-cascade">
              Excluir também o que depende dela (CASCADE)
            </Label>
          </div>
        )}

        <SqlPreview {...preview} />

        <AlertDialogFooter>
          <Button variant="ghost" size="sm" onClick={close}>
            Cancelar
          </Button>
          <Button
            variant="destructive"
            size="sm"
            disabled={!canApply}
            onClick={() => void submit()}
          >
            {isApplying ? 'Excluindo…' : 'Excluir'}
          </Button>
        </AlertDialogFooter>
      </AlertDialogContent>
    </AlertDialog>
  );
}
