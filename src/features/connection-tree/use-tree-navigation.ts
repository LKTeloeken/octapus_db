import type { Virtualizer } from '@tanstack/react-virtual';
import { useCallback, useEffect, useMemo, type RefObject } from 'react';
import { useShortcuts } from '@/lib/keybindings/use-shortcuts';
import { useFocusStore } from '@/stores/focus-store';
import { useTreeStore } from '@/stores/tree-store';
import type { FlatRow } from './connection-tree.types';

interface Options {
  rows: FlatRow[];
  virtualizer: Virtualizer<HTMLDivElement, Element>;
  containerRef: RefObject<HTMLDivElement | null>;
}

/** Linhas que o cursor do teclado visita: nós e o "carregar mais". */
const isNavigable = (row: FlatRow) =>
  row.variant === 'node' || row.variant === 'more';

const rowLevel = (row: FlatRow) =>
  row.variant === 'node' ? row.props.level : 'level' in row ? row.level : 0;

/** Primeira linha navegável (pulando erros e filtros) andando em `dir` a partir de `from`. */
const findNode = (rows: FlatRow[], from: number, dir: 1 | -1): number => {
  for (let i = from; i >= 0 && i < rows.length; i += dir) {
    if (isNavigable(rows[i])) return i;
  }
  return -1;
};

/**
 * O filtro que vale para a linha `index`: o do próprio nó (logo abaixo dele,
 * se expandido com filtro) ou o da lista em que ela está — que fica no topo
 * da lista, no mesmo nível, antes de chegar ao nó pai.
 */
const findFilter = (rows: FlatRow[], index: number): number => {
  const next = rows[index + 1];
  if (next?.variant === 'filter' && next.nodeId === rows[index].id)
    return index + 1;
  const level = rowLevel(rows[index]);
  for (let i = index; i >= 0; i--) {
    const row = rows[i];
    if (row.variant === 'filter' && row.level === level) return i;
    if (row.variant === 'node' && row.props.level < level) return -1;
  }
  return -1;
};

/**
 * Navegação por teclado da árvore virtualizada, no mesmo molde da
 * use-palette-navigation: a lista já vem achatada, então basta um cursor sobre
 * ela. O cursor é guardado por **nodeId**, não por índice — expandir/colapsar
 * reescreve a lista inteira e um índice cru escorregaria para outro nó.
 */
export const useTreeNavigation = ({
  rows,
  virtualizer,
  containerRef,
}: Options) => {
  const focusedNodeId = useTreeStore(state => state.focusedNodeId);
  const setFocusedNode = useTreeStore(state => state.setFocusedNode);
  const toggleNode = useTreeStore(state => state.toggleNode);
  const expandNode = useTreeStore(state => state.expandNode);
  const collapseNode = useTreeStore(state => state.collapseNode);
  const focusRequest = useFocusStore(state => state.request);
  const requestFocus = useFocusStore(state => state.requestFocus);
  const clearFocusRequest = useFocusStore(state => state.clearFocusRequest);

  // Só linhas de nó entram no cursor: as de erro têm id próprio e não têm
  // `props`, então nunca podem virar a posição ativa.
  const activeIndex = useMemo(() => {
    if (focusedNodeId === null) return -1;
    const index = rows.findIndex(row => row.id === focusedNodeId);
    return index !== -1 && isNavigable(rows[index]) ? index : -1;
  }, [rows, focusedNodeId]);

  const focusIndex = useCallback(
    (index: number) => {
      const row = rows[index];
      if (!row || !isNavigable(row)) return;
      setFocusedNode(row.id);
      virtualizer.scrollToIndex(index, { align: 'auto' });
    },
    [rows, setFocusedNode, virtualizer],
  );

  /** Leva o foco ao campo de filtro da linha `index` (montando-o se preciso). */
  const focusFilter = useCallback(
    (index: number) => {
      const row = rows[index];
      if (row?.variant !== 'filter') return;
      virtualizer.scrollToIndex(index, { align: 'auto' });
      requestAnimationFrame(() => {
        containerRef.current
          ?.querySelector<HTMLInputElement>(
            `[data-tree-filter="${CSS.escape(row.nodeId)}"]`,
          )
          ?.focus();
      });
    },
    [rows, virtualizer, containerRef],
  );

  /** Sai do filtro de volta para a árvore, no primeiro filho filtrado. */
  const leaveFilter = useCallback(
    (nodeId: string) => {
      const filterIndex = rows.findIndex(
        row => row.variant === 'filter' && row.nodeId === nodeId,
      );
      containerRef.current?.focus();
      const first =
        filterIndex === -1 ? -1 : findNode(rows, filterIndex + 1, 1);
      if (first !== -1) focusIndex(first);
    },
    [rows, containerRef, focusIndex],
  );

  const move = useCallback(
    (dir: 1 | -1) => {
      const start =
        activeIndex < 0 ? (dir === 1 ? -1 : rows.length) : activeIndex;
      const next = findNode(rows, start + dir, dir);
      if (next === -1) return;
      focusIndex(next);
    },
    [activeIndex, rows, focusIndex],
  );

  // Atalhos globais de painel. Moram aqui porque a árvore é o único painel
  // sempre montado do shell — o ResultsTable vai e volta com as abas.
  // Cmd/Ctrl+B volta para a árvore de qualquer lugar; Cmd/Ctrl+←/→ segue a
  // geometria da tela (árvore à esquerda, grade à direita) e, num campo de
  // texto ou no CodeMirror, fica com o "início/fim da linha".
  useShortcuts({
    'app.focusTree': () => requestFocus('tree'),
    'app.focusLeft': () => requestFocus('tree'),
    'app.focusRight': () => requestFocus('grid'),
  });

  // Consome os pedidos endereçados à árvore.
  useEffect(() => {
    if (focusRequest?.panel !== 'tree') return;
    containerRef.current?.focus();
    clearFocusRequest();
  }, [focusRequest, containerRef, clearFocusRequest]);

  // A árvore recebeu o foco de volta: cancela um pedido pendente para a grade,
  // senão uma tabela que ainda estava carregando roubaria o teclado depois.
  const onFocus = useCallback(() => {
    if (focusRequest?.panel === 'grid') clearFocusRequest();
  }, [focusRequest, clearFocusRequest]);

  /**
   * Uma ação sobre a linha do cursor. Sem cursor ainda (ou o nó sumiu da
   * lista), a primeira tecla de navegação pousa no primeiro nó.
   */
  const atCursor =
    (action: (row: FlatRow) => void | boolean) => (): void | boolean => {
      if (activeIndex === -1) {
        const first = findNode(rows, 0, 1);
        if (first !== -1) focusIndex(first);
        return;
      }
      return action(rows[activeIndex]);
    };

  /** Sobe para o pai: a linha de nó de nível menor logo acima */
  const focusParent = (level: number) => {
    for (let i = activeIndex - 1; i >= 0; i--) {
      const candidate = rows[i];
      if (candidate.variant === 'node' && candidate.props.level < level) {
        focusIndex(i);
        return;
      }
    }
  };

  // As teclas (setas e hjkl, Enter e o, gg, G, /) vêm do registro de atalhos.
  // Letras num campo (o filtro, que já para a propagação) são texto: o
  // `useShortcuts` não dá a elas as ações da árvore.
  const { onKeyDown } = useShortcuts(
    {
      'tree.down': atCursor(() => move(1)),
      'tree.up': atCursor(() => move(-1)),

      // Colapsado com filhos → expande; já expandido → desce para o filho.
      'tree.expand': atCursor(row => {
        if (row.variant !== 'node') return false;
        if (row.props.hasChildren && !row.props.isExpanded) {
          expandNode(row.id);
        } else if (row.props.isExpanded) {
          move(1);
        }
      }),

      // Expandido → colapsa; senão → sobe para o pai. No "carregar mais", sobe.
      'tree.collapse': atCursor(row => {
        if (row.variant === 'more') {
          focusParent(row.level);
          return;
        }
        if (row.variant !== 'node') return false;
        if (row.props.isExpanded) collapseNode(row.id);
        else focusParent(row.props.level);
      }),

      // No "carregar mais", pede a próxima janela
      'tree.open': atCursor(row => {
        if (row.variant === 'more') {
          row.onMore();
          return;
        }
        if (row.variant !== 'node') return false;
        if (row.onEnter) row.onEnter();
        else if (row.props.hasChildren) toggleNode(row.id);
      }),

      'tree.first': () => {
        const first = findNode(rows, 0, 1);
        if (first !== -1) focusIndex(first);
      },
      'tree.last': () => {
        const last = findNode(rows, rows.length - 1, -1);
        if (last !== -1) focusIndex(last);
      },

      'tree.filter': () => {
        if (activeIndex === -1) return false;
        const filterIndex = findFilter(rows, activeIndex);
        if (filterIndex === -1) return false;
        focusFilter(filterIndex);
      },

      'tree.blur': () => {
        if (activeIndex === -1) return false;
        containerRef.current?.blur();
      },
    },
    { target: 'element' },
  );

  return { focusedNodeId, setFocusedNode, onKeyDown, onFocus, leaveFilter };
};
