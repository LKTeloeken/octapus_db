import type { ReactNode } from 'react';

export interface TooltipProps {
  position?: 'top' | 'bottom' | 'left' | 'right';
  content: string;
  children: ReactNode;
  className?: string;
  delayDuration?: number;
  arrow?: boolean;
  /** Distância do trigger; o padrão reserva espaço para a seta */
  sideOffset?: number;
}
