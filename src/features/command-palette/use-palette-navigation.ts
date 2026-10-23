import type { Virtualizer } from '@tanstack/react-virtual';
import { useCallback, useEffect, useState } from 'react';
import { useShortcuts } from '@/lib/keybindings/use-shortcuts';
import type { PaletteRow } from './command-palette.types';

type ItemRow = Extract<PaletteRow, { kind: 'item' }>;

interface Options {
  rows: PaletteRow[];
  virtualizer: Virtualizer<HTMLDivElement, Element>;
  onSelect: (row: ItemRow) => void;
  /** Fixar o schema do item ativo (`null` sem item) */
  onPin: (row: ItemRow | null) => void;
  onUnpin: () => void;
}

/** First selectable (item) row index walking `dir` from `from`, or -1. */
const findItem = (rows: PaletteRow[], from: number, dir: 1 | -1): number => {
  for (let i = from; i >= 0 && i < rows.length; i += dir) {
    if (rows[i].kind === 'item') return i;
  }
  return -1;
};

/**
 * Self-managed keyboard navigation for the virtualized palette. Tracks the
 * active row, skips group headers with the arrow keys, scrolls off-screen rows
 * into view, and selects on Enter — the role cmdk used to play via the DOM.
 */
export const usePaletteNavigation = ({
  rows,
  virtualizer,
  onSelect,
  onPin,
  onUnpin,
}: Options) => {
  const [activeIndex, setActiveIndex] = useState(-1);

  // New result set → highlight the first item and scroll back to the top
  // (offset 0 keeps the leading group header visible).
  useEffect(() => {
    setActiveIndex(findItem(rows, 0, 1));
    virtualizer.scrollToOffset(0);
  }, [rows, virtualizer]);

  const move = useCallback(
    (dir: 1 | -1) => {
      const start =
        activeIndex < 0 ? (dir === 1 ? -1 : rows.length) : activeIndex;
      const next = findItem(rows, start + dir, dir);
      if (next === -1) return;
      setActiveIndex(next);
      // Reaching the first item: scroll to the top so its header stays visible.
      if (next === findItem(rows, 0, 1)) virtualizer.scrollToOffset(0);
      else virtualizer.scrollToIndex(next, { align: 'auto' });
    },
    [activeIndex, rows, virtualizer],
  );

  // Tab fixa um schema e Shift+Tab solta (padrões): o foco nunca sai do
  // campo — a paleta não tem outro controle focável.
  const { onKeyDown } = useShortcuts(
    {
      'palette.down': () => move(1),
      'palette.up': () => move(-1),
      'palette.select': () => {
        const row = rows[activeIndex];
        if (row?.kind !== 'item') return false;
        onSelect(row);
      },
      'palette.pinSchema': () => {
        const row = rows[activeIndex];
        onPin(row?.kind === 'item' ? row : null);
      },
      'palette.unpinSchema': onUnpin,
    },
    { target: 'element' },
  );

  return { activeIndex, setActiveIndex, onKeyDown };
};
