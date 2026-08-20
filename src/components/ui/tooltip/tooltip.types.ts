import type { ReactNode } from 'react';

export interface TooltipProps {
  position?: 'top' | 'bottom' | 'left' | 'right';
  content: string;
  children: ReactNode;
  className?: string;
  delayDuration?: number;
  arrow?: boolean;
}
