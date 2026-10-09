import type { Server } from '@/api/types/server.types';
import type { NodeRowProps } from './node-row/node-row.types';

export interface ConnectionTreeProps {
  /** Opens the server form in edit mode (owned by the sidebar) */
  onEditServer: (server: Server) => void;
}

/**
 * A single flattened, virtualizable tree row. The whole visible tree is
 * collapsed into a flat list so only the on-screen rows hit the DOM — the
 * array itself may hold thousands of entries (multi-tenant databases).
 */
export type FlatRow =
  | {
      variant: 'node';
      id: string;
      props: NodeRowProps;
      /** Ação do `Enter` quando o nó difere do clique: numa tabela o clique
       *  também alterna as colunas, o teclado só abre a aba de browse. */
      onEnter?: () => void;
    }
  | {
      variant: 'error';
      id: string;
      level: number;
      message: string;
      onRetry: () => void;
    }
  | {
      /** Filtro dos filhos de um nó grande (schemas de tenant, tabelas) */
      variant: 'filter';
      id: string;
      level: number;
      /** Nó cujos filhos o filtro restringe */
      nodeId: string;
      value: string;
      /** Filhos que passam no filtro */
      total: number;
      placeholder: string;
      onChange: (value: string) => void;
    }
  | {
      /** A janela mostra só parte dos filhos: pede a próxima */
      variant: 'more';
      id: string;
      level: number;
      remaining: number;
      isLoading: boolean;
      onMore: () => void;
    };
