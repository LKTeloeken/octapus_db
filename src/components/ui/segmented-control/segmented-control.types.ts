import type { ReactNode } from 'react';

export interface SegmentedControlOption<T extends string> {
  value: T;
  label: ReactNode;
}

export interface SegmentedControlProps<T extends string> {
  value: T;
  onValueChange: (value: T) => void;
  options: SegmentedControlOption<T>[];
  /** `xs` (22 px, status bar) ou `sm` (28 px, toolbars). */
  size?: 'xs' | 'sm';
  /** Nome do grupo para leitores de tela. */
  'aria-label': string;
  className?: string;
}

export interface ThumbRect {
  left: number;
  width: number;
}
