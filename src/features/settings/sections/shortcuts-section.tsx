import {
  Add01Icon,
  Cancel01Icon,
  RotateLeft01Icon,
  Search01Icon,
} from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input/input';
import { Kbd } from '@/components/ui/kbd';
import { cn } from '@/lib/utils';
import {
  useShortcutsSection,
  type ShortcutRowView,
} from './use-shortcuts-section';

export function ShortcutsSection() {
  const section = useShortcutsSection();
  const { query, setQuery, groups, hasCustomizations, isSaving, restoreAll } =
    section;

  return (
    <div className="flex flex-col gap-5">
      <div className="flex items-center gap-2">
        <Input
          size="sm"
          aria-label="Buscar atalho"
          placeholder="Buscar ação ou tecla"
          className="flex-1"
          value={query}
          onChange={event => setQuery(event.target.value)}
          InputProps={{
            startAdornment: (
              <HugeiconsIcon icon={Search01Icon} className="text-fg-subtle" />
            ),
          }}
        />
        <Button
          variant="ghost"
          disabled={!hasCustomizations || isSaving}
          onClick={restoreAll}
        >
          Restaurar padrões
        </Button>
      </div>

      {groups.length === 0 && (
        <p className="text-small text-fg-muted">Nenhum atalho com “{query}”.</p>
      )}

      {groups.map(({ group, rows }) => (
        <section key={group} className="flex flex-col">
          <h4 className="pb-1 text-small font-medium text-fg-subtle">
            {group}
          </h4>
          {rows.map(row => (
            <ShortcutRow key={row.id} row={row} section={section} />
          ))}
        </section>
      ))}
    </div>
  );
}

interface ShortcutRowProps {
  row: ShortcutRowView;
  section: ReturnType<typeof useShortcutsSection>;
}

function ShortcutRow({ row, section }: ShortcutRowProps) {
  const {
    recordingId,
    recordingLabel,
    startRecording,
    cancelRecording,
    conflict,
    replaceConflict,
    dismissConflict,
    error,
    removeBinding,
    resetAction,
    isSaving,
  } = section;
  const isRecording = recordingId === row.id;
  const rowConflict = conflict?.id === row.id ? conflict : null;
  const rowError = error?.id === row.id ? error.message : null;

  return (
    <div className="flex flex-col gap-1.5 border-b border-line-subtle py-2 last:border-b-0">
      <div className="flex items-center justify-between gap-4">
        <span className="min-w-0 text-body text-fg">{row.label}</span>

        <div className="flex shrink-0 flex-wrap items-center justify-end gap-1.5">
          {row.bindings.length === 0 && !isRecording && (
            <span className="text-small text-fg-subtle">Sem atalho</span>
          )}

          {row.bindings.map(({ binding, label }) => (
            <span
              key={binding}
              className="group relative inline-flex items-center"
            >
              <Kbd>{label}</Kbd>
              {/* Por cima do canto da tecla: escondido, não ocupa espaço */}
              <button
                type="button"
                aria-label={`Remover ${label}`}
                title={`Remover ${label}`}
                disabled={isSaving}
                className="absolute -top-1.5 -right-1.5 flex size-3.5 cursor-pointer items-center justify-center rounded-full bg-surface-3 text-fg-muted opacity-0 shadow-[inset_0_0_0_1px_var(--line-strong)] transition-opacity outline-none group-hover:opacity-100 hover:text-fg focus-visible:opacity-100 focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-ring"
                onClick={() => removeBinding(row.id, binding)}
              >
                <HugeiconsIcon icon={Cancel01Icon} className="size-2.5" />
              </button>
            </span>
          ))}

          {isRecording ? (
            <span className="inline-flex items-center gap-1">
              <Kbd
                className={cn(
                  'px-1.5 text-iris-text',
                  !recordingLabel && 'font-sans',
                )}
              >
                {recordingLabel ?? 'Pressione as teclas…'}
              </Kbd>
              <Button
                variant="ghost"
                size="icon-xs"
                aria-label="Cancelar a gravação"
                title="Cancelar (esc)"
                onClick={cancelRecording}
              >
                <HugeiconsIcon icon={Cancel01Icon} />
              </Button>
            </span>
          ) : (
            <Button
              variant="ghost"
              size="icon-xs"
              aria-label={`Adicionar tecla para ${row.label}`}
              title="Adicionar tecla"
              disabled={isSaving}
              onClick={() => startRecording(row.id)}
            >
              <HugeiconsIcon icon={Add01Icon} />
            </Button>
          )}

          {row.customized && (
            <Button
              variant="ghost"
              size="icon-xs"
              aria-label={`Restaurar o padrão de ${row.label}`}
              title="Restaurar o padrão"
              disabled={isSaving}
              onClick={() => resetAction(row.id)}
            >
              <HugeiconsIcon icon={RotateLeft01Icon} />
            </Button>
          )}
        </div>
      </div>

      {row.notes.map(note => (
        <p key={note} className="text-small text-fg-muted">
          {note}
        </p>
      ))}

      {rowError && <p className="text-small text-danger">{rowError}</p>}

      {rowConflict && (
        <div className="flex items-center justify-between gap-3 rounded-sm bg-warning-soft px-2.5 py-1.5">
          <p className="text-small text-fg">{rowConflict.message}</p>
          <div className="flex shrink-0 gap-1">
            <Button variant="ghost" size="xs" onClick={dismissConflict}>
              Cancelar
            </Button>
            <Button variant="outline" size="xs" onClick={replaceConflict}>
              Substituir
            </Button>
          </div>
        </div>
      )}
    </div>
  );
}
