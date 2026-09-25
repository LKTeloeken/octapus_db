import type { IconSvgElement } from '@hugeicons/react';
import type { NodeKind } from '@/lib/node-ref';

export interface NodeRowAction {
  label: string;
  icon: IconSvgElement;
  onSelect: () => void;
}

export interface NodeRowProps {
  level: number;
  kind: NodeKind;
  name: string;
  /** Secondary text (column type, table count...) */
  subLabel?: string;
  /** Tamanho em disco; quando presente ocupa o lugar do botão de ações
   *  (que continuam no clique direito) */
  sizeBytes?: number | null;
  hasChildren: boolean;
  isExpanded?: boolean;
  isLoading?: boolean;
  /** Highlights the kind icon (e.g. connected server) */
  isHighlighted?: boolean;
  /** Nó sob o cursor do teclado (setas na árvore) */
  isFocused?: boolean;
  onClick?: () => void;
  actions?: NodeRowAction[];
}
