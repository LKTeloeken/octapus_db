import {
  Cancel01Icon,
  Copy01Icon,
  MinusSignIcon,
  SquareIcon,
} from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { memo } from 'react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import { useTrafficLights, useWindowControls } from './use-window-controls';

/** Linux: minimizar, maximizar/restaurar e fechar, no fim da linha de abas. */
export const WindowControls = memo(() => {
  const { isMaximized, minimize, toggleMaximize, close } = useWindowControls();

  return (
    <div className="flex shrink-0 items-center gap-0.5">
      <Button
        variant="ghost"
        size="icon"
        aria-label="Minimizar"
        onClick={minimize}
        className="text-fg-subtle"
      >
        <HugeiconsIcon icon={MinusSignIcon} className="size-3.5" />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        aria-label={isMaximized ? 'Restaurar' : 'Maximizar'}
        onClick={toggleMaximize}
        className="text-fg-subtle"
      >
        <HugeiconsIcon
          icon={isMaximized ? Copy01Icon : SquareIcon}
          className="size-3.5"
        />
      </Button>
      <Button
        variant="ghost"
        size="icon"
        aria-label="Fechar"
        onClick={close}
        className="text-fg-subtle hover:bg-danger-soft hover:text-danger"
      >
        <HugeiconsIcon icon={Cancel01Icon} className="size-3.5" />
      </Button>
    </div>
  );
});

WindowControls.displayName = 'WindowControls';

// Os símbolos do semáforo imitam os do sistema (8 px, traço fino) — não são
// ícones de interface, por isso não vêm do Hugeicons.
const CloseGlyph = () => (
  <svg
    viewBox="0 0 8 8"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.2"
    strokeLinecap="round"
  >
    <path d="M2 2l4 4M6 2L2 6" />
  </svg>
);

const MinimizeGlyph = () => (
  <svg
    viewBox="0 0 8 8"
    fill="none"
    stroke="currentColor"
    strokeWidth="1.2"
    strokeLinecap="round"
  >
    <path d="M1.6 4h4.8" />
  </svg>
);

const FullscreenGlyph = () => (
  <svg viewBox="0 0 8 8" fill="currentColor">
    <path d="M2 2h3.2L2 5.2z" />
    <path d="M6 6H2.8L6 2.8z" />
  </svg>
);

const light =
  'inline-flex size-3 cursor-default items-center justify-center rounded-full bg-traffic-rest text-transparent shadow-[inset_0_0_0_0.5px_var(--traffic-edge)] transition-colors outline-none focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-solid focus-visible:outline-ring [&_svg]:size-2';

/**
 * macOS: fechar, minimizar e tela cheia à esquerda. Cinza em repouso; ao
 * passar o mouse no grupo (ou focar um deles pelo teclado), as três ganham as cores.
 */
export const TrafficLights = memo(() => {
  const { minimize, toggleFullscreen, close } = useTrafficLights();

  return (
    <div
      role="group"
      aria-label="Controles da janela"
      className="group/traffic flex shrink-0 items-center gap-2"
    >
      <button
        type="button"
        aria-label="Fechar"
        onClick={close}
        className={cn(
          light,
          'group-hover/traffic:bg-traffic-close group-hover/traffic:text-traffic-glyph group-has-focus-visible/traffic:bg-traffic-close group-has-focus-visible/traffic:text-traffic-glyph',
        )}
      >
        <CloseGlyph />
      </button>
      <button
        type="button"
        aria-label="Minimizar"
        onClick={minimize}
        className={cn(
          light,
          'group-hover/traffic:bg-traffic-minimize group-hover/traffic:text-traffic-glyph group-has-focus-visible/traffic:bg-traffic-minimize group-has-focus-visible/traffic:text-traffic-glyph',
        )}
      >
        <MinimizeGlyph />
      </button>
      <button
        type="button"
        aria-label="Tela cheia"
        onClick={toggleFullscreen}
        className={cn(
          light,
          'group-hover/traffic:bg-traffic-zoom group-hover/traffic:text-traffic-glyph group-has-focus-visible/traffic:bg-traffic-zoom group-has-focus-visible/traffic:text-traffic-glyph',
        )}
      >
        <FullscreenGlyph />
      </button>
    </div>
  );
});

TrafficLights.displayName = 'TrafficLights';
