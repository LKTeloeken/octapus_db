import { HugeiconsIcon, type IconSvgElement } from '@hugeicons/react';
import type { ReactNode } from 'react';
import { OctapusIdle } from '@/components/octapus-idle';
import { cn } from '@/lib/utils';

interface ContentStateProps {
  icon: IconSvgElement;
  title: ReactNode;
  description?: ReactNode;
  /** Ação principal (botão) abaixo do texto */
  action?: ReactNode;
  /** `danger` pinta o selo do ícone para estados de erro */
  tone?: 'neutral' | 'danger';
  /**
   * Tela vazia grande (sem abas): o polvo no lugar do ícone, título em
   * `display` e tudo alinhado à esquerda no terço de cima
   */
  mascot?: boolean;
  className?: string;
}

/**
 * Estado vazio / erro (DESIGN.md, Estados vazios): selo redondo com o ícone,
 * título na voz da marca (Bricolage), texto de apoio e uma ação clara —
 * nunca uma linha cinza solta.
 */
export function ContentState({
  icon,
  title,
  description,
  action,
  tone = 'neutral',
  mascot = false,
  className,
}: ContentStateProps) {
  if (mascot) {
    return (
      <div
        className={cn(
          'flex h-full w-full justify-center p-8 pt-[18vh]',
          className,
        )}
      >
        <div className="flex w-full max-w-md flex-col items-start gap-3">
          <OctapusIdle className="mb-1 h-[72px] w-[84px]" />
          <p className="font-display text-display font-bold text-fg">{title}</p>
          {description && (
            <div className="max-w-[40ch] text-body text-fg-muted">
              {description}
            </div>
          )}
          {action && (
            <div className="mt-2 flex w-full max-w-xs flex-col gap-0.5">
              {action}
            </div>
          )}
        </div>
      </div>
    );
  }

  return (
    <div
      className={cn(
        'flex h-full w-full items-center justify-center p-6',
        className,
      )}
    >
      <div className="flex max-w-md flex-col items-center gap-3 text-center">
        <span
          className={cn(
            'flex size-11 items-center justify-center rounded-full',
            tone === 'danger'
              ? 'bg-danger-soft text-danger'
              : 'bg-tinta-soft text-tinta-text',
          )}
        >
          <HugeiconsIcon icon={icon} className="size-[22px]" />
        </span>
        <p className="font-display text-title font-semibold text-fg">{title}</p>
        {description && (
          <div className="text-body text-fg-muted">{description}</div>
        )}
        {action && (
          <div className="mt-1 flex flex-wrap justify-center gap-2">
            {action}
          </div>
        )}
      </div>
    </div>
  );
}
