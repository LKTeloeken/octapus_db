import type { Virtualizer } from '@tanstack/react-virtual';
import { useCallback, useEffect, useMemo, type KeyboardEvent, type RefObject } from 'react';
import { useFocusStore } from '@/stores/focus-store';
import { useTreeStore } from '@/stores/tree-store';
import type { FlatRow } from './connection-tree.types';

type NodeRow = Extract<FlatRow, { variant: 'node' }>;

interface Options {
  rows: FlatRow[];
  virtualizer: Virtualizer<HTMLDivElement, Element>;
  containerRef: RefObject<HTMLDivElement | null>;
}

/** Primeira linha de nó (pulando erros) andando em `dir` a partir de `from`. */
const findNode = (rows: FlatRow[], from: number, dir: 1 | -1): number => {
  for (let i = from; i >= 0 && i < rows.length; i += dir) {
    if (rows[i].variant === 'node') return i;
  }
  return -1;
};

/**
 * Navegação por teclado da árvore virtualizada, no mesmo molde da
 * use-palette-navigation: a lista já vem achatada, então basta um cursor sobre
 * ela. O cursor é guardado por **nodeId**, não por índice — expandir/colapsar
 * reescreve a lista inteira e um índice cru escorregaria para outro nó.
 */
export const useTreeNavigation = ({ rows, virtualizer, containerRef }: Options) => {
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
    return index !== -1 && rows[index].variant === 'node' ? index : -1;
  }, [rows, focusedNodeId]);

  const focusIndex = useCallback(
    (index: number) => {
      const row = rows[index];
      if (!row || row.variant !== 'node') return;
      setFocusedNode(row.id);
      virtualizer.scrollToIndex(index, { align: 'auto' });
    },
    [rows, setFocusedNode, virtualizer],
  );

  const move = useCallback(
    (dir: 1 | -1) => {
      const start = activeIndex < 0 ? (dir === 1 ? -1 : rows.length) : activeIndex;
      const next = findNode(rows, start + dir, dir);
      if (next === -1) return;
      focusIndex(next);
    },
    [activeIndex, rows, focusIndex],
  );

  // Atalhos globais de painel. Moram aqui porque a árvore é o único painel
  // sempre montado do shell — o ResultsTable vai e volta com as abas.
  //
  // Cmd/Ctrl+B: volta para a árvore de qualquer lugar (sem guard de digitação,
  // como o Cmd+K do palette — nenhum editor do app usa esse acorde).
  // Cmd/Ctrl+←/→: segue a geometria da tela (árvore à esquerda, grade à
  // direita). Esse par PRECISA do guard: em campo de texto e no CodeMirror ele
  // é "ir para o início/fim da linha".
  useEffect(() => {
    const handler = (event: globalThis.KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey)) return;

      if (event.key.toLowerCase() === 'b') {
        event.preventDefault();
        requestFocus('tree');
        return;
      }

      if (event.key !== 'ArrowLeft' && event.key !== 'ArrowRight') return;

      const target = event.target as HTMLElement | null;
      const isTyping =
        !!target &&
        (target.tagName === 'INPUT' ||
          target.tagName === 'TEXTAREA' ||
          target.isContentEditable);
      if (isTyping) return;

      event.preventDefault();
      requestFocus(event.key === 'ArrowLeft' ? 'tree' : 'grid');
    };

    window.addEventListener('keydown', handler);
    return () => window.removeEventListener('keydown', handler);
  }, [requestFocus]);

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

  const onKeyDown = useCallback(
    (event: KeyboardEvent) => {
      // Acordes com modificador pertencem aos atalhos globais (Cmd/Ctrl+←/→),
      // que não podem também colapsar/expandir o nó sob o cursor.
      if (event.metaKey || event.ctrlKey || event.altKey) return;

      // Sem cursor ainda (ou o nó sumiu da lista): a primeira tecla de
      // navegação pousa no primeiro nó.
      if (activeIndex === -1) {
        if (
          event.key === 'ArrowDown' ||
          event.key === 'ArrowUp' ||
          event.key === 'ArrowRight' ||
          event.key === 'ArrowLeft' ||
          event.key === 'Enter'
        ) {
          event.preventDefault();
          const first = findNode(rows, 0, 1);
          if (first !== -1) focusIndex(first);
        }
        return;
      }

      const row = rows[activeIndex] as NodeRow;

      switch (event.key) {
        case 'ArrowDown':
          event.preventDefault();
          move(1);
          return;

        case 'ArrowUp':
          event.preventDefault();
          move(-1);
          return;

        // Colapsado com filhos → expande; já expandido → desce para o filho.
        case 'ArrowRight':
          event.preventDefault();
          if (row.props.hasChildren && !row.props.isExpanded) {
            expandNode(row.id);
          } else if (row.props.isExpanded) {
            move(1);
          }
          return;

        // Expandido → colapsa; senão → sobe para o pai (nível menor acima).
        case 'ArrowLeft': {
          event.preventDefault();
          if (row.props.isExpanded) {
            collapseNode(row.id);
            return;
          }
          for (let i = activeIndex - 1; i >= 0; i--) {
            const candidate = rows[i];
            if (
              candidate.variant === 'node' &&
              candidate.props.level < row.props.level
            ) {
              focusIndex(i);
              return;
            }
          }
          return;
        }

        case 'Enter':
          event.preventDefault();
          if (row.onEnter) row.onEnter();
          else if (row.props.hasChildren) toggleNode(row.id);
          return;

        case 'Escape':
          containerRef.current?.blur();
          return;
      }
    },
    [
      activeIndex,
      rows,
      move,
      focusIndex,
      expandNode,
      collapseNode,
      toggleNode,
      containerRef,
    ],
  );

  return { focusedNodeId, setFocusedNode, onKeyDown, onFocus };
};
