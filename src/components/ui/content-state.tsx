import { HugeiconsIcon, type IconSvgElement } from '@hugeicons/react';
import type { ReactNode } from 'react';
import { cn } from '@/lib/utils';

interface ContentStateProps {
  icon: IconSvgElement;
  title: ReactNode;
  description?: ReactNode;
  /** Ação principal (botão) abaixo do texto */
  action?: ReactNode;
  /** `danger` pinta o quadro do ícone para estados de erro */
  tone?: 'neutral' | 'danger';
  className?: string;
}

/**
 * Estado vazio / erro (DESIGN.md §8): ícone num quadro de 44 px, título,
 * texto de apoio e uma ação clara — nunca uma linha cinza solta.
 */
export function ContentState({
  icon,
  title,
  description,
  action,
  tone = 'neutral',
  className,
}: ContentStateProps) {
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
            'flex size-11 items-center justify-center rounded-lg',
            tone === 'danger'
              ? 'bg-danger-soft text-danger'
              : 'bg-hover text-fg-muted shadow-[inset_0_0_0_1px_var(--line-subtle)]',
          )}
        >
          <HugeiconsIcon icon={icon} className="size-[22px]" />
        </span>
        <p className="text-title font-semibold text-fg">{title}</p>
        {description && (
          <div className="text-body text-fg-muted">{description}</div>
        )}
        {action && <div className="mt-1 flex flex-wrap justify-center gap-2">{action}</div>}
      </div>
    </div>
  );
}
