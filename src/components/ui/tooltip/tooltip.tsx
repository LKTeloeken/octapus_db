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

export const TooltipProvider = Provider;

export function Tooltip({
  position = 'top',
  content,
  children,
  className,
  delayDuration = 0,
  arrow = true,
}: TooltipProps) {
  return (
    <Root delayDuration={delayDuration}>
      <Trigger asChild>
        <span className="inline-flex max-w-full">{children}</span>
      </Trigger>

      <Portal>
        <Content
          className={cn(
            'bg-popover text-popover-foreground animate-in fade-in-0 zoom-in-95 data-[state=closed]:animate-out data-[state=closed]:fade-out-0 data-[state=closed]:zoom-out-95 data-[side=bottom]:slide-in-from-top-2 data-[side=left]:slide-in-from-right-2 data-[side=right]:slide-in-from-left-2 data-[side=top]:slide-in-from-bottom-2 z-50 w-fit origin-(---tooltip-content-transform-origin) rounded-md px-3 py-1.5 text-xs text-balance',
            className,
          )}
          side={position}
        >
          {content}
          {arrow && (
            <Arrow className="bg-primary fill-primary z-50 size-2.5 translate-y-[calc(-50%-2px)] rotate-45 rounded-xs" />
          )}
        </Content>
      </Portal>
    </Root>
  );
}
