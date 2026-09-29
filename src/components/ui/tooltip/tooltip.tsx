import type { TooltipProps } from './tooltip.types';
import {
  Portal,
  Content,
  Root,
  Trigger,
  Arrow,
  Provider,
} from '@radix-ui/react-tooltip';
import { cn } from '@/lib/utils';
import { TOOLTIP_MOTION } from '../motion';

export const TooltipProvider = Provider;

/** Sólido (superfície 3), 12 px, sem seta por padrão; fade curto de 2 px. */
export function Tooltip({
  position = 'top',
  content,
  children,
  className,
  // Sem valor: herda o atraso do TooltipProvider (app.tsx).
  delayDuration,
  arrow = false,
  sideOffset,
}: TooltipProps) {
  return (
    <Root delayDuration={delayDuration}>
      <Trigger asChild>
        <span className="inline-flex max-w-full">{children}</span>
      </Trigger>

      <Portal>
        <Content
          className={cn(
            'z-50 inline-flex w-fit min-h-7 items-center gap-2 origin-(--radix-tooltip-content-transform-origin) rounded-sm border border-line bg-surface-3 px-2.5 py-1 text-small text-fg text-balance shadow-overlay',
            TOOLTIP_MOTION,
            className,
          )}
          side={position}
          sideOffset={sideOffset ?? 6}
        >
          {content}
          {arrow && (
            <Arrow className="z-50 size-2.5 translate-y-[calc(-50%_-_2px)] rotate-45 rounded-xs bg-surface-3 fill-surface-3" />
          )}
        </Content>
      </Portal>
    </Root>
  );
}
