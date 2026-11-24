import * as React from 'react';

import { cn } from '@/lib/utils';

/**
 * Tecla de atalho (`⌘K`, `esc`, `↵`): mono 11 px, 18 px de altura.
 * `inverse` é a versão para dentro do botão primário (sobre Tinta).
 */
function Kbd({
  className,
  variant = 'default',
  ...props
}: React.ComponentProps<'kbd'> & { variant?: 'default' | 'inverse' }) {
  return (
    <kbd
      data-slot="kbd"
      className={cn(
        'inline-flex shrink-0 items-center justify-center rounded-xs px-1 font-mono text-[11px] leading-none',
        variant === 'inverse'
          ? // Só contorno: um fundo claro sobre Tinta derrubaria o contraste do texto.
            // Na cor de `--on-tinta` (branco ou o abismo, conforme o acento)
            'h-4 min-w-4 text-on-tinta ring-1 ring-current/40 ring-inset'
          : 'h-[18px] min-w-[18px] bg-hover text-fg-muted shadow-[inset_0_0_0_1px_var(--line-subtle),inset_0_-1px_0_var(--line)]',
        className,
      )}
      {...props}
    />
  );
}

export { Kbd };
