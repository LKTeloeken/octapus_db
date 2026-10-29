import {
  useCallback,
  useEffect,
  useRef,
  useState,
  type RefObject,
} from 'react';

/** Quanto o ponteiro anda antes de virar arraste (abaixo disso é clique) */
const DRAG_THRESHOLD_PX = 5;
/** Perto da borda da lista, o arraste rola as abas escondidas para a vista */
const EDGE_SCROLL_PX = 32;
const EDGE_SCROLL_STEP = 12;

export interface TabDrag {
  tabId: string;
  /** Posição de inserção entre as abas atuais (0…n, contando a arrastada) */
  dropIndex: number;
}

interface Pending {
  tabId: string;
  startX: number;
  dragging: boolean;
}

/**
 * Reordenar abas arrastando. Com ponteiro, e não com o drag-and-drop do HTML:
 * o WebView do Tauri intercepta o arrastar nativo para soltar arquivos.
 *
 * Cada aba leva `data-tab-id` no seu contêiner; o ponto de inserção é o número
 * de abas cujo meio fica à esquerda do ponteiro. O clique que encerra um
 * arraste é engolido para não acionar o que estiver embaixo.
 */
export const useTabReorder = (
  listRef: RefObject<HTMLDivElement | null>,
  moveTab: (tabId: string, toIndex: number) => void,
) => {
  const [drag, setDrag] = useState<TabDrag | null>(null);
  const pending = useRef<Pending | null>(null);
  const cleanup = useRef<(() => void) | null>(null);

  useEffect(() => () => cleanup.current?.(), []);

  const dropIndexAt = useCallback(
    (clientX: number) => {
      const list = listRef.current;
      if (!list) return 0;
      const tabs = list.querySelectorAll<HTMLElement>('[data-tab-id]');
      let index = 0;
      tabs.forEach(tab => {
        const rect = tab.getBoundingClientRect();
        if (rect.left + rect.width / 2 < clientX) index += 1;
      });
      return index;
    },
    [listRef],
  );

  const onPointerDown = useCallback(
    (tabId: string, event: React.PointerEvent) => {
      if (event.button !== 0 || pending.current) return;
      // O "x" de fechar é só clique
      if ((event.target as HTMLElement).closest('[data-tab-close]')) return;
      pending.current = { tabId, startX: event.clientX, dragging: false };

      const onMove = (move: PointerEvent) => {
        const state = pending.current;
        if (!state) return;
        if (
          !state.dragging &&
          Math.abs(move.clientX - state.startX) < DRAG_THRESHOLD_PX
        ) {
          return;
        }
        state.dragging = true;
        document.body.style.cursor = 'grabbing';

        const list = listRef.current;
        if (list) {
          const bounds = list.getBoundingClientRect();
          if (move.clientX < bounds.left + EDGE_SCROLL_PX) {
            list.scrollLeft -= EDGE_SCROLL_STEP;
          } else if (move.clientX > bounds.right - EDGE_SCROLL_PX) {
            list.scrollLeft += EDGE_SCROLL_STEP;
          }
        }
        setDrag({ tabId: state.tabId, dropIndex: dropIndexAt(move.clientX) });
      };

      const finish = (commit: boolean) => (end: PointerEvent) => {
        const state = pending.current;
        cleanup.current?.();
        if (!state?.dragging) return;

        if (commit) {
          const ids = Array.from(
            listRef.current?.querySelectorAll<HTMLElement>('[data-tab-id]') ??
              [],
            tab => tab.dataset.tabId,
          );
          const from = ids.indexOf(state.tabId);
          const drop = dropIndexAt(end.clientX);
          // Tirar a aba da lista desloca as que vêm depois dela
          const to = drop > from ? drop - 1 : drop;
          if (from !== -1 && to !== from) moveTab(state.tabId, to);
        }

        const swallowClick = (click: MouseEvent) => {
          click.stopPropagation();
          click.preventDefault();
        };
        window.addEventListener('click', swallowClick, {
          capture: true,
          once: true,
        });
        // Sem clique depois (soltou fora da aba), o engolidor não pode ficar
        setTimeout(
          () =>
            window.removeEventListener('click', swallowClick, {
              capture: true,
            }),
          0,
        );
      };

      const onUp = finish(true);
      const onCancel = finish(false);
      window.addEventListener('pointermove', onMove);
      window.addEventListener('pointerup', onUp);
      window.addEventListener('pointercancel', onCancel);
      cleanup.current = () => {
        window.removeEventListener('pointermove', onMove);
        window.removeEventListener('pointerup', onUp);
        window.removeEventListener('pointercancel', onCancel);
        document.body.style.cursor = '';
        pending.current = null;
        cleanup.current = null;
        setDrag(null);
      };
    },
    [listRef, moveTab, dropIndexAt],
  );

  return { drag, onPointerDown };
};
