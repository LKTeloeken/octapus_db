import { Add01Icon, Delete02Icon } from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
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
import { SqlPreview } from './sql-preview';
import type { DialogOf } from './structure-editor.types';
import { TypeDatalist } from './type-datalist';
import { typeListId } from './type-suggestions';
import { useTableFormDialog } from './use-table-form-dialog';

const GRID =
  'grid grid-cols-[minmax(0,1.3fr)_minmax(0,1fr)_minmax(0,1fr)_2.5rem_2.5rem_1.75rem] items-center gap-2';

interface TableFormDialogProps {
  open: boolean;
  dialog: DialogOf<'createTable'>;
}

/** Criar tabela: nome, colunas (tipo, default, nulo, chave) e o SQL */
export function TableFormDialog({ open, dialog }: TableFormDialogProps) {
  const {
    name,
    setName,
    columns,
    updateColumn,
    addColumn,
    removeColumn,
    preview,
    canApply,
    isApplying,
    submit,
    close,
  } = useTableFormDialog(dialog);
  const { target } = dialog;

  return (
    <Dialog open={open} onOpenChange={next => !next && close()}>
      <DialogContent className="sm:max-w-2xl">
        <DialogHeader>
          <DialogTitle>Nova tabela</DialogTitle>
          <DialogDescription>
            Em{' '}
            <span className="font-mono text-fg">
              {target.schema ?? target.database}
            </span>
            {target.schema && (
              <>
                {' '}
                · <span className="font-mono">{target.database}</span>
              </>
            )}
          </DialogDescription>
        </DialogHeader>

        <form
          className="flex min-w-0 flex-col gap-5"
          onSubmit={event => {
            event.preventDefault();
            void submit();
          }}
        >
          <Input
            label="Nome da tabela"
            placeholder="pedidos"
            autoFocus
            value={name}
            onChange={event => setName(event.target.value)}
          />

          <div className="flex min-w-0 flex-col gap-2">
            <div
              className={`${GRID} text-micro font-medium uppercase text-fg-subtle`}
            >
              <span>Coluna</span>
              <span>Tipo</span>
              <span>Default</span>
              <span className="text-center">Nulo</span>
              <span className="text-center">PK</span>
              <span />
            </div>

            <div className="flex max-h-64 flex-col gap-2 overflow-y-auto scrollbar-thin">
              {columns.map((column, index) => (
                <div key={column.key} className={GRID}>
                  <Input
                    size="sm"
                    aria-label={`Nome da coluna ${index + 1}`}
                    placeholder="nome"
                    inputClassName="font-mono"
                    value={column.name}
                    onChange={event =>
                      updateColumn(column.key, { name: event.target.value })
                    }
                  />
                  <Input
                    size="sm"
                    aria-label={`Tipo da coluna ${index + 1}`}
                    placeholder="tipo"
                    list={typeListId(target.dbType)}
                    inputClassName="font-mono"
                    value={column.dataType}
                    onChange={event =>
                      updateColumn(column.key, { dataType: event.target.value })
                    }
                  />
                  <Input
                    size="sm"
                    aria-label={`Default da coluna ${index + 1}`}
                    placeholder="sem default"
                    inputClassName="font-mono"
                    value={column.defaultValue ?? ''}
                    onChange={event =>
                      updateColumn(column.key, {
                        defaultValue: event.target.value || null,
                      })
                    }
                  />
                  <span className="flex justify-center">
                    <Checkbox
                      aria-label={`Coluna ${index + 1} aceita nulo`}
                      checked={column.nullable}
                      // Chave primária nunca é nula
                      disabled={column.primaryKey}
                      onCheckedChange={checked =>
                        updateColumn(column.key, { nullable: checked === true })
                      }
                    />
                  </span>
                  <span className="flex justify-center">
                    <Checkbox
                      aria-label={`Coluna ${index + 1} na chave primária`}
                      checked={column.primaryKey}
                      onCheckedChange={checked =>
                        updateColumn(column.key, {
                          primaryKey: checked === true,
                          ...(checked === true && { nullable: false }),
                        })
                      }
                    />
                  </span>
                  <Button
                    type="button"
                    variant="ghost"
                    size="icon"
                    aria-label={`Remover a coluna ${index + 1}`}
                    disabled={columns.length === 1}
                    onClick={() => removeColumn(column.key)}
                  >
                    <HugeiconsIcon icon={Delete02Icon} />
                  </Button>
                </div>
              ))}
            </div>

            <Button
              type="button"
              variant="ghost"
              size="sm"
              className="self-start"
              onClick={addColumn}
            >
              <HugeiconsIcon icon={Add01Icon} />
              Adicionar coluna
            </Button>
          </div>

          <SqlPreview {...preview} />
          <TypeDatalist dbType={target.dbType} />

          {/* Enter em qualquer campo cria a tabela */}
          <button type="submit" hidden />
        </form>

        <DialogFooter>
          <Button variant="ghost" size="sm" onClick={close}>
            Cancelar
          </Button>
          <Button size="sm" disabled={!canApply} onClick={() => void submit()}>
            {isApplying ? 'Criando…' : 'Criar tabela'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
