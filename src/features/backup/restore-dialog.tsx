import type { ReactNode } from 'react';
import { Button } from '@/components/ui/button';
import {
  Dialog,
  DialogContent,
  DialogDescription,
  DialogFooter,
  DialogHeader,
  DialogTitle,
} from '@/components/ui/dialog';
import { Spinner } from '@/components/ui/spinner';
import type { ArchiveSummary } from '@/api/types/backup.types';
import { formatBytes } from '@/lib/format-bytes';
import type { BackupDialog } from '@/stores/backup-dialog-store';
import { CommandPreview } from './command-preview';
import { OptionRow } from './option-row';
import { useRestoreDialog } from './use-restore-dialog';

interface RestoreDialogProps {
  open: boolean;
  dialog: Extract<BackupDialog, { kind: 'restore' }>;
}

const plural = (count: number, one: string, many: string) =>
  `${count} ${count === 1 ? one : many}`;

export function RestoreDialog({ open, dialog }: RestoreDialogProps) {
  const {
    database,
    path,
    fileName,
    chooseFile,
    file,
    isInspecting,
    inspectError,
    options,
    setOptions,
    isCheckingTools,
    toolsProblem,
    preview,
    confirming,
    cancelConfirm,
    canStart,
    isStarting,
    submit,
    close,
  } = useRestoreDialog(dialog);
  const custom = file?.format === 'custom';

  return (
    <Dialog open={open} onOpenChange={next => !next && close()}>
      <DialogContent>
        <DialogHeader>
          <DialogTitle>
            Restaurar em <span className="font-mono">{database}</span>
          </DialogTitle>
          <DialogDescription>
            O conteúdo do arquivo é criado neste database. Aceita backups .dump
            (pg_dump) e scripts .sql.
          </DialogDescription>
        </DialogHeader>

        <form
          className="flex flex-col gap-4"
          onSubmit={event => {
            event.preventDefault();
            void submit();
          }}
        >
          <div className="flex items-center justify-between gap-3 rounded-sm border border-line bg-field px-3 py-2.5">
            <div className="flex min-w-0 flex-col gap-0.5">
              {fileName ? (
                <span
                  className="truncate font-mono text-body text-fg"
                  title={path ?? undefined}
                >
                  {fileName}
                </span>
              ) : (
                <span className="text-body text-fg-muted">
                  Nenhum arquivo escolhido
                </span>
              )}
              {file && (
                <span className="text-small text-fg-muted">
                  {custom ? 'Backup do PostgreSQL (.dump)' : 'Script SQL'} ·{' '}
                  {formatBytes(file.sizeBytes)}
                </span>
              )}
              {isInspecting && (
                <span className="inline-flex items-center gap-1.5 text-small text-fg-muted">
                  <Spinner className="size-3" /> Lendo o arquivo…
                </span>
              )}
            </div>
            <Button
              variant={path ? 'ghost' : 'outline'}
              size="sm"
              onClick={() => void chooseFile()}
            >
              {path ? 'Trocar…' : 'Escolher arquivo…'}
            </Button>
          </div>

          {inspectError && <Problem>{inspectError}</Problem>}
          {toolsProblem && <Problem>{toolsProblem}</Problem>}
          {isCheckingTools && !toolsProblem && (
            <p className="inline-flex items-center gap-1.5 text-small text-fg-muted">
              <Spinner className="size-3" /> Verificando o pg_restore…
            </p>
          )}

          {file?.summary && <SummaryView summary={file.summary} />}
          {custom && file?.summaryError && (
            <Problem>
              O pg_restore não conseguiu ler o arquivo: {file.summaryError}
            </Problem>
          )}
          {file?.format === 'plain' && (
            <p className="text-small text-fg-muted">
              Roda pelo psql. Dono, permissões e DROP são os que estão escritos
              no script.
            </p>
          )}

          {file && (
            <div className="flex flex-col gap-3">
              {custom && (
                <>
                  <OptionRow
                    id="restore-no-owner"
                    label="Ignorar dono e permissões"
                    hint="Os objetos ficam com o seu usuário; sem os GRANTs do arquivo."
                    checked={options.noOwner}
                    onChange={noOwner => setOptions({ noOwner })}
                  />
                  <OptionRow
                    id="restore-clean"
                    label="Apagar objetos existentes antes"
                    hint="DROP de cada objeto do arquivo que já existe aqui — os dados atuais dele se perdem."
                    danger
                    checked={options.clean}
                    onChange={clean => setOptions({ clean })}
                  />
                </>
              )}
              <OptionRow
                id="restore-single-transaction"
                label="Tudo ou nada"
                hint="Uma transação só: no primeiro erro, nada fica. Desligado, os erros são pulados e listados no fim."
                checked={options.singleTransaction}
                onChange={singleTransaction =>
                  setOptions({ singleTransaction })
                }
              />
            </div>
          )}

          {file && (
            <CommandPreview
              command={preview.command}
              error={preview.error}
              isLoading={preview.isLoading}
            />
          )}

          {confirming && (
            <Problem>
              Os objetos do arquivo que já existem em{' '}
              <span className="font-mono">{database}</span> vão ser apagados
              antes de recriados. Não dá para desfazer.
            </Problem>
          )}
        </form>

        <DialogFooter>
          <Button
            variant="ghost"
            size="sm"
            onClick={confirming ? cancelConfirm : close}
          >
            {confirming ? 'Voltar' : 'Cancelar'}
          </Button>
          <Button
            size="sm"
            variant={confirming ? 'destructive' : 'default'}
            disabled={!canStart}
            onClick={() => void submit()}
          >
            {isStarting && <Spinner className="size-3" />}
            {confirming ? 'Apagar e restaurar' : 'Restaurar'}
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}

function Problem({ children }: { children: ReactNode }) {
  return (
    <p
      role="alert"
      className="rounded-sm bg-danger-soft px-2.5 py-2 text-small break-words text-danger"
    >
      {children}
    </p>
  );
}

/** O que há no `.dump`: origem e contagens */
function SummaryView({ summary }: { summary: ArchiveSummary }) {
  const origin = [
    summary.database && `de ${summary.database}`,
    summary.serverVersion && `servidor ${summary.serverVersion.split(' ')[0]}`,
    summary.pgDumpVersion && `pg_dump ${summary.pgDumpVersion}`,
  ].filter(Boolean);
  const counts = [
    plural(summary.schemaCount, 'schema', 'schemas'),
    plural(summary.tableCount, 'tabela', 'tabelas'),
    summary.viewCount > 0 && plural(summary.viewCount, 'view', 'views'),
    summary.hasData ? 'com dados' : 'só estrutura',
  ].filter(Boolean);
  const more = summary.schemaCount - summary.schemas.length;

  return (
    <div className="flex flex-col gap-1 text-small text-fg-muted">
      <p>
        <span className="text-fg">{counts.join(' · ')}</span>
        {origin.length > 0 && ` — ${origin.join(', ')}`}
      </p>
      {summary.schemas.length > 0 && (
        <p className="truncate font-mono" title={summary.schemas.join(', ')}>
          {summary.schemas.join(', ')}
          {more > 0 && ` e mais ${more}`}
        </p>
      )}
    </div>
  );
}
