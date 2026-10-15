import type { IconSvgElement } from '@hugeicons/react';
import type { CatalogNodeKind } from '@/api/types/catalog.types';
import type { DatabaseType } from '@/api/types/server.types';
import type { NodeKind } from '@/lib/node-ref';

export interface NodeRowAction {
  label: string;
  icon: IconSvgElement;
  onSelect: () => void;
  /** Ação destrutiva (excluir): texto e ícone em `--danger` */
  danger?: boolean;
  /** Divisória antes da ação (separa grupos: navegar, estrutura, excluir) */
  separated?: boolean;
}

/** Os nós do catálogo mais o grupo de servidores (que fica acima do servidor) */
export type NodeRowKind = NodeKind | 'group';

export interface NodeRowProps {
  level: number;
  kind: NodeRowKind;
  name: string;
  /** Secondary text (column type, table count...) */
  subLabel?: string;
  /** Tamanho em disco; quando presente ocupa o lugar do botão de ações
   *  (que continuam no clique direito) */
  sizeBytes?: number | null;
  hasChildren: boolean;
  isExpanded?: boolean;
  isLoading?: boolean;
  /** Banco do servidor: pinta o ícone com a cor de identidade (só no servidor) */
  dbType?: DatabaseType;
  /** Aviso curto ao lado do nome (tenant fora do formato dominante: `−3`),
   *  com a explicação no `title` */
  badge?: { label: string; title: string };
  /** Tipo da relação, quando o catálogo informa (view e particionada têm ícone próprio) */
  relationKind?: CatalogNodeKind;
  /** Tabela aberta na aba ativa: a linha fica realçada (`--active`) */
  isOpen?: boolean;
  /** Nó sob o cursor do teclado (setas na árvore) */
  isFocused?: boolean;
  onClick?: () => void;
  actions?: NodeRowAction[];
}
