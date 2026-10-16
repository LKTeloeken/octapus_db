import { Button } from '@/components/ui/button';
import { Checkbox } from '@/components/ui/checkbox';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Input } from '@/components/ui/input/input';
import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import { SqlPreview } from './sql-preview';
import type { DialogOf } from './structure-editor.types';
import { TypeDatalist } from './type-datalist';
import { typeListId } from './type-suggestions';
import { useColumnFormDialog } from './use-column-form-dialog';

interface ColumnFormDialogProps {
  open: boolean;
  dialog: DialogOf<'addColumn'> | DialogOf<'editColumn'>;
}

/** Nova coluna numa tabela, ou edição de uma (nome, tipo, nulo, default) */
export function ColumnFormDialog({ open, dialog }: ColumnFormDialogProps) {
  const {
    isEdit,
    column,
    setField,
    rebuilds,
    preview,
    canApply,
    isApplying,
    submit,
    close,
  } = useColumnFormDialog(dialog);
  const { target, table } = dialog;
  const where = target.schema ? `${target.schema}.${table}` : table;

  return (
    <Dialog open={open} onOpenChange={next => !next && close()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>
            {dialog.kind === 'editColumn' ? (
              <>
                Editar <span className="font-mono">{dialog.column.name}</span>
              </>
            ) : (
              'Nova coluna'
            )}
          </DialogTitle>
          <DialogDescription>
            Em <span className="font-mono text-fg">{where}</span>
          </DialogDescription>
        </DialogHeader>

        <form
          className="flex flex-col gap-5"
          onSubmit={event => {
            event.preventDefault();
            void submit();
          }}
        >
          <div className="flex gap-2">
            <Input
              label="Nome"
              placeholder="status"
              autoFocus
              className="flex-1"
              inputClassName="font-mono"
              value={column.name}
              onChange={event => setField('name', event.target.value)}
            />
            <Input
              label="Tipo"
              placeholder={target.dbType === 'sqlite' ? 'TEXT' : 'text'}
              list={typeListId(target.dbType)}
              className="flex-1"
              inputClassName="font-mono"
              value={column.dataType}
              onChange={event => setField('dataType', event.target.value)}
            />
          </div>

          <Input
            label="Default"
            placeholder="sem default — ex.: 0, 'ativo', now()"
            inputClassName="font-mono"
            value={column.defaultValue ?? ''}
            onChange={event =>
              setField('defaultValue', event.target.value || null)
            }
          />

          <div className="flex flex-wrap items-center gap-x-6 gap-y-3">
            <div className="flex items-center gap-2">
              <Switch
                id="column-nullable"
                checked={column.nullable}
                disabled={column.primaryKey}
                onCheckedChange={checked => setField('nullable', checked)}
              />
              <Label htmlFor="column-nullable">Aceita nulo</Label>
            </div>
            {/* A chave só se define ao criar: mudar a PK é outra operação */}
            {!isEdit && (
              <div className="flex items-center gap-2">
                <Checkbox
                  id="column-primary-key"
                  checked={column.primaryKey}
                  onCheckedChange={checked => {
                    setField('primaryKey', checked === true);
                    if (checked === true) setField('nullable', false);
                  }}
                />
                <Label htmlFor="column-primary-key">Chave primária</Label>
              </div>
            )}
          </div>

          {rebuilds && (
            <p className="rounded-sm bg-info-soft px-2.5 py-2 text-small text-info">
              O SQLite não altera tipo, nulo nem default de uma coluna: a tabela
              é recriada com os mesmos dados, índices e triggers.
            </p>
          )}

          <SqlPreview {...preview} />
          <TypeDatalist dbType={target.dbType} />
          <button type="submit" hidden />
        </form>

        <DialogFooter>
          <Button variant="ghost" size="sm" onClick={close}>
            Cancelar
          </Button>
          <Button size="sm" disabled={!canApply} onClick={() => void submit()}>
            {isApplying ? 'Salvando…' : isEdit ? 'Salvar' : 'Adicionar coluna'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
