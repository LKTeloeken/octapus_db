import { HugeiconsIcon } from '@hugeicons/react';
import { ServerStack01Icon } from '@hugeicons/core-free-icons';
import { memo } from 'react';
import { OctapusIdle } from '@/components/octapus-idle';
import { OctapusThinking } from '@/components/octapus-thinking/octapus-thinking';
import { Kbd } from '@/components/ui/kbd';
import { Spinner } from '@/components/ui/spinner';
import { DB_TYPE_TEXT_COLOR } from '@/lib/db-defaults';
import { useShortcutLabel } from '@/lib/keybindings/use-shortcuts';
import { cn } from '@/lib/utils';
import { useCommandPaletteStore } from '@/stores/command-palette-store';
import { useStatusBar } from './use-status-bar';

/** Separador da barra: uma ventosa de 3 px */
const Sep = () => (
  <span aria-hidden className="size-[3px] shrink-0 rounded-full bg-fg-subtle" />
);

/**
 * Barra de status (DESIGN.md, Barra de status): 32 px direto sobre o abismo.
 * Contexto à esquerda (polvo, conexão da aba ativa) e o que o app está
 * fazendo; linhas e tempo ficam no rodapé da grade, sem repetir aqui. O chip da conexão pulsa em biolume só
 * enquanto a consulta roda; conexão de produção leva `danger-soft`.
 */
export const StatusBar = memo(() => {
  const status = useStatusBar();
  const paletteKey = useShortcutLabel('app.commandPalette');
  const setPaletteOpen = useCommandPaletteStore(state => state.setOpen);
  const showMascot =
    status.mascot === 'always' ||
    (status.mascot === 'busy' && status.anyRunning);

  return (
    <footer
      data-tauri-drag-region
      className="flex h-8 shrink-0 items-center gap-2 px-2.5 text-small text-fg-muted"
    >
      {showMascot &&
        (status.anyRunning ? (
          <OctapusThinking
            className="size-4 text-tinta-text"
            label="Trabalhando"
          />
        ) : (
          <OctapusIdle className="size-4" />
        ))}

      {status.connection && (
        <span
          className={cn(
            'inline-flex h-[22px] min-w-0 items-center gap-1.5 rounded-full px-2 text-fg shadow-[inset_0_0_0_1px_var(--line-subtle)]',
            status.connection.production
              ? 'bg-danger-soft'
              : status.running
                ? 'animate-live bg-biolume-soft'
                : 'bg-surface-1',
          )}
        >
          <HugeiconsIcon
            icon={ServerStack01Icon}
            className={cn(
              'size-3.5 shrink-0',
              DB_TYPE_TEXT_COLOR[status.connection.dbType],
            )}
          />
          <span className="truncate">{status.connection.serverName}</span>
          <Sep />
          <span className="truncate">{status.connection.database}</span>
          {status.connection.production && (
            <span className="font-semibold text-danger">PRODUÇÃO</span>
          )}
        </span>
      )}

      <span className="flex min-w-0 flex-1 items-center gap-1.5 truncate">
        {status.running ? (
          <>
            <Spinner className="size-3.5 text-tinta-text" />
            <span>
              Executando
              {status.elapsed !== null &&
                ` · ${status.elapsed.toFixed(1).replace('.', ',')} s`}
            </span>
          </>
        ) : (
          status.connection && <span className="text-fg-subtle">Pronto</span>
        )}
      </span>

      <button
        type="button"
        onClick={() => setPaletteOpen(true)}
        aria-label="Abrir a paleta de comandos"
        title="Paleta de comandos"
        className="inline-flex cursor-pointer items-center rounded-sm px-1 py-0.5 transition-colors hover:bg-hover hover:text-fg"
      >
        {paletteKey ? <Kbd>{paletteKey}</Kbd> : 'Paleta'}
      </button>
    </footer>
  );
});

StatusBar.displayName = 'StatusBar';
