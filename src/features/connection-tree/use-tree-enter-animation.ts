import { useCallback, useEffect, useRef, useState } from 'react';
import { useTreeStore } from '@/stores/tree-store';
import type { FlatRow } from './connection-tree.types';

/** Quanto tempo depois de expandir os filhos ainda entram animados (a carga pode demorar). */
const ENTER_WINDOW_MS = 1500;
/** Escalonamento entre linhas, só nas 10 primeiras (DESIGN.md §6). */
const STAGGER_MS = 16;
const STAGGER_MAX = 10;

/**
 * Entrada dos filhos ao expandir um nó: y −4→0 + fade em 180 ms, +16 ms por
 * linha. Só os descendentes de um nó expandido há pouco animam — rolar a árvore
 * virtualizada monta linhas o tempo todo e elas não podem piscar.
 */
export const useTreeEnterAnimation = (rows: FlatRow[]) => {
  const expanded = useTreeStore(state => state.expanded);
  const previousRef = useRef(expanded);
  const [recent, setRecent] = useState<string[]>([]);

  useEffect(() => {
    const previous = previousRef.current;
    previousRef.current = expanded;
    const added = Array.from(expanded).filter(id => !previous.has(id));
    if (added.length === 0) return;

    setRecent(added);
    const timer = setTimeout(() => setRecent([]), ENTER_WINDOW_MS);
    return () => clearTimeout(timer);
  }, [expanded]);

  /** Atraso da animação da linha, ou `null` quando ela não deve animar. */
  const getEnterDelay = useCallback(
    (index: number): number | null => {
      if (recent.length === 0) return null;
      const id = rows[index]?.id;
      if (!id) return null;

      const parentId = recent.find(parent => id.startsWith(`${parent}|`));
      if (!parentId) return null;

      const parentIndex = rows.findIndex(row => row.id === parentId);
      const position = Math.max(0, index - parentIndex - 1);
      return Math.min(position, STAGGER_MAX) * STAGGER_MS;
    },
    [recent, rows],
  );

  return { getEnterDelay };
};
