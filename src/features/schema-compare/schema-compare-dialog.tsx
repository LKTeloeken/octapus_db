import {
  ArrowDataTransferHorizontalIcon,
  CheckmarkCircle02Icon,
} from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import type { ReactNode } from 'react';
import { Badge } from '@/components/ui/badge';
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
import { Spinner } from '@/components/ui/spinner';
import { formatCount } from '@/lib/format-count';
import { describeChange, type ChangeTone } from './change-labels';
import {
  useSchemaCompareDialog,
  type CompareDialog,
} from './use-schema-compare-dialog';

const TONE_BADGE: Record<ChangeTone, 'danger' | 'info' | 'warning'> = {
  onlyBase: 'danger',
  onlyOther: 'info',
  changed: 'warning',
};

function Section({
  title,
  children,
}: {
  title: ReactNode;
  children: ReactNode;
}) {
  return (
    <section className="flex flex-col gap-2">
      <h3 className="text-micro font-medium uppercase text-fg-subtle">
        {title}
      </h3>
      {children}
    </section>
  );
}

function TableList({
  tables,
  variant,
}: {
  tables: string[];
  variant: 'danger' | 'info';
}) {
  return (
    <div className="flex flex-wrap gap-1.5">
      {tables.map(table => (
        <Badge key={table} variant={variant} mono>
          {table}
        </Badge>
      ))}
    </div>
  );
}

interface SchemaCompareDialogProps {
  open: boolean;
  dialog: CompareDialog;
}

/**
 * Compara dois schemas: as tabelas que só um tem e, nas comuns, as colunas
 * que diferem (tipo, nulo, default), da base para o outro.
 */
export function SchemaCompareDialog({
  open,
  dialog,
}: SchemaCompareDialogProps) {
  const {
    base,
    setBase,
    other,
    setOther,
    baseSuggestions,
    otherSuggestions,
    swap,
    canCompare,
    compare,
    result,
    isComparing,
    error,
    close,
  } = useSchemaCompareDialog(dialog);

  const identical =
    !!result &&
    !result.onlyInBase.length &&
    !result.onlyInOther.length &&
    !result.different.length;

  return (
    <Dialog open={open} onOpenChange={next => !next && close()}>
      <DialogContent className="sm:max-w-xl">
        <DialogHeader>
          <DialogTitle>Comparar schemas</DialogTitle>
          <DialogDescription>
            Em <span className="font-mono text-fg">{dialog.database}</span>:
            tabelas e colunas do outro schema contra a base.
          </DialogDescription>
        </DialogHeader>

        <form
          className="flex items-end gap-2"
          onSubmit={event => {
            event.preventDefault();
            compare();
          }}
        >
          <Input
            label="Base"
            placeholder="schema"
            autoFocus={!base}
            className="flex-1"
            inputClassName="font-mono"
            list="compare-base-schemas"
            value={base}
            onChange={event => setBase(event.target.value)}
          />
          <Button
            type="button"
            variant="ghost"
            size="icon"
            className="mb-0.5"
            aria-label="Inverter base e outro"
            onClick={swap}
          >
            <HugeiconsIcon icon={ArrowDataTransferHorizontalIcon} />
          </Button>
          <Input
            label="Comparar com"
            placeholder="schema"
            autoFocus={!!base}
            className="flex-1"
            inputClassName="font-mono"
            list="compare-other-schemas"
            value={other}
            onChange={event => setOther(event.target.value)}
          />
          <datalist id="compare-base-schemas">
            {baseSuggestions.map(name => (
              <option key={name} value={name} />
            ))}
          </datalist>
          <datalist id="compare-other-schemas">
            {otherSuggestions.map(name => (
              <option key={name} value={name} />
            ))}
          </datalist>
          <button type="submit" hidden />
        </form>

        {(isComparing || error || result) && (
          <div className="-mx-6 max-h-[55vh] overflow-y-auto border-t border-line-subtle px-6 pt-4 scrollbar-thin">
            {isComparing ? (
              <p className="flex items-center gap-2 text-body text-fg-muted">
                <Spinner className="size-3.5 text-fg-subtle" />
                Comparando…
              </p>
            ) : error ? (
              <p className="text-body text-danger">{error}</p>
            ) : result && identical ? (
              <p className="flex items-center gap-2 text-body text-fg-muted">
                <HugeiconsIcon
                  icon={CheckmarkCircle02Icon}
                  className="size-4 text-success"
                />
                Mesmas tabelas e colunas nos dois ({formatCount(result.common)}{' '}
                {result.common === 1 ? 'tabela' : 'tabelas'}).
              </p>
            ) : result ? (
              <div className="flex flex-col gap-5">
                <p className="text-small text-fg-subtle tabular-nums">
                  {formatCount(result.common)} em comum ·{' '}
                  {formatCount(result.different.length)} com colunas diferentes
                </p>
                {result.onlyInBase.length > 0 && (
                  <Section
                    title={
                      <>
                        Só em{' '}
                        <span className="font-mono normal-case">
                          {result.base}
                        </span>
                      </>
                    }
                  >
                    <TableList tables={result.onlyInBase} variant="danger" />
                  </Section>
                )}
                {result.onlyInOther.length > 0 && (
                  <Section
                    title={
                      <>
                        Só em{' '}
                        <span className="font-mono normal-case">
                          {result.other}
                        </span>
                      </>
                    }
                  >
                    <TableList tables={result.onlyInOther} variant="info" />
                  </Section>
                )}
                {result.different.length > 0 && (
                  <Section title="Colunas diferentes">
                    <ul className="flex flex-col gap-3">
                      {result.different.map(({ table, changes }) => (
                        <li key={table} className="flex flex-col gap-1">
                          <span className="font-mono text-body font-medium text-fg">
                            {table}
                          </span>
                          {changes.map(change => {
                            const { tone, label, detail } =
                              describeChange(change);
                            return (
                              <div
                                key={change.column}
                                className="flex items-center gap-2 pl-3 text-small"
                              >
                                <Badge variant={TONE_BADGE[tone]}>
                                  {label}
                                </Badge>
                                <span className="shrink-0 font-mono text-fg">
                                  {change.column}
                                </span>
                                <span
                                  className="min-w-0 truncate font-mono text-fg-muted"
                                  title={detail}
                                >
                                  {detail}
                                </span>
                              </div>
                            );
                          })}
                        </li>
                      ))}
                    </ul>
                  </Section>
                )}
              </div>
            ) : null}
          </div>
        )}

        <DialogFooter>
          <Button variant="ghost" size="sm" onClick={close}>
            Fechar
          </Button>
          <Button
            size="sm"
            disabled={!canCompare || isComparing}
            onClick={compare}
          >
            Comparar
          </Button>
        </DialogFooter>
      </DialogContent>
    </Dialog>
  );
}
