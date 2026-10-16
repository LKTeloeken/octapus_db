import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { SegmentedControl } from '@/components/ui/segmented-control/segmented-control';
import { Spinner } from '@/components/ui/spinner';
import type { BackupDialog as BackupDialogState } from '@/stores/backup-dialog-store';
import {
  CONTENT_OPTIONS,
  FORMAT_HINTS,
  FORMAT_OPTIONS,
} from './backup-options';
import { CommandPreview } from './command-preview';
import { OptionRow } from './option-row';
import { useBackupDialog } from './use-backup-dialog';

interface BackupDialogProps {
  open: boolean;
  dialog: Extract<BackupDialogState, { kind: 'backup' }>;
}

export function BackupDialog({ open, dialog }: BackupDialogProps) {
  const {
    options,
    setOptions,
    title,
    isTable,
    tools,
    preview,
    canStart,
    isStarting,
    submit,
    close,
  } = useBackupDialog(dialog);
  const plain = options.format === 'plain';

  return (
    <Dialog open={open} onOpenChange={next => !next && close()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>
            Backup de <span className="font-mono">{title}</span>
          </DialogTitle>
          <DialogDescription>
            {tools.isLoading ? (
              <span className="inline-flex items-center gap-1.5">
                <Spinner className="size-3" /> Verificando o pg_dump…
              </span>
            ) : tools.clientVersion && tools.serverVersion ? (
              `pg_dump ${tools.clientVersion} · servidor ${tools.serverVersion}`
            ) : (
              'Gerado pelo pg_dump, sem travar as tabelas.'
            )}
          </DialogDescription>
        </DialogHeader>

        <form
          className="flex flex-col gap-4"
          onSubmit={event => {
            event.preventDefault();
            void submit();
          }}
        >
          {(tools.problem || tools.error) && (
            <p
              role="alert"
              className="rounded-sm bg-danger-soft px-2.5 py-2 text-small text-danger"
            >
              {tools.problem ?? tools.error}
            </p>
          )}

          <div className="flex flex-col gap-1.5">
            <span className="text-small font-medium text-fg-muted">
              Formato
            </span>
            <SegmentedControl
              size="sm"
              aria-label="Formato"
              className="self-start"
              value={options.format}
              onValueChange={format => setOptions({ format })}
              options={FORMAT_OPTIONS}
            />
            <p className="text-small text-fg-muted">
              {FORMAT_HINTS[options.format]}
            </p>
          </div>

          <div className="flex flex-col gap-1.5">
            <span className="text-small font-medium text-fg-muted">
              Conteúdo
            </span>
            <SegmentedControl
              size="sm"
              aria-label="Conteúdo"
              className="self-start"
              value={options.content}
              onValueChange={content => setOptions({ content })}
              options={CONTENT_OPTIONS}
            />
          </div>

          <div className="flex flex-col gap-3">
            <OptionRow
              id="backup-no-owner"
              label="Sem dono e permissões"
              hint="Para restaurar em outro servidor, com outros usuários."
              checked={options.noOwner}
              onChange={noOwner => setOptions({ noOwner })}
            />
            {plain && (
              <>
                <OptionRow
                  id="backup-clean"
                  label="Apagar antes de criar"
                  hint="DROP … IF EXISTS antes de cada objeto, para rodar por cima do que existe."
                  checked={options.clean}
                  disabled={options.content === 'dataOnly'}
                  onChange={clean => setOptions({ clean })}
                />
                <OptionRow
                  id="backup-inserts"
                  label="INSERT em vez de COPY"
                  hint="Mais lento, mas roda em ferramentas que não entendem COPY."
                  checked={options.columnInserts}
                  disabled={options.content === 'schemaOnly'}
                  onChange={columnInserts => setOptions({ columnInserts })}
                />
              </>
            )}
          </div>

          {isTable && (
            <p className="text-small text-fg-muted">
              O schema, tipos, funções e extensões que a tabela usa não vão
              junto — o destino precisa tê-los.
            </p>
          )}

          <CommandPreview
            command={preview.command}
            error={preview.error}
            isLoading={preview.isLoading}
          />
        </form>

        <DialogFooter>
          <Button variant="ghost" size="sm" onClick={close}>
            Cancelar
          </Button>
          <Button size="sm" disabled={!canStart} onClick={() => void submit()}>
            {isStarting && <Spinner className="size-3" />}
            Salvar backup…
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
