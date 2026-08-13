import { create } from 'zustand';

/** Painéis que disputam o foco do teclado no shell */
export type FocusPanel = 'tree' | 'grid';

/**
 * Pedido de foco entre painéis. É um *request*, não um "painel atual": quem
 * pede não conhece o DOM do outro lado, e o alvo pode nem estar montado ainda
 * (abrir uma tabela cria a aba antes de os dados chegarem). Cada painel observa
 * o pedido, foca a si mesmo quando puder e o consome.
 *
 * O `id` existe porque pedir o mesmo painel duas vezes seguidas precisa reemitir
 * — abrir duas tabelas em sequência é exatamente esse caso.
 */
interface FocusState {
  request: { panel: FocusPanel; id: number } | null;
  requestFocus: (panel: FocusPanel) => void;
  clearFocusRequest: () => void;
}

export const useFocusStore = create<FocusState>((set, get) => ({
  request: null,

  requestFocus: panel => {
    const previousId = get().request?.id ?? 0;
    set({ request: { panel, id: previousId + 1 } });
  },

  clearFocusRequest: () => {
    if (get().request === null) return;
    set({ request: null });
  },
}));
