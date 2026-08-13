import type { Virtualizer } from '@tanstack/react-virtual';
import { useCallback, useEffect, useState, type KeyboardEvent } from 'react';
import type { PaletteRow } from './command-palette.types';

type ItemRow = Extract<PaletteRow, { kind: 'item' }>;

interface Options {
  rows: PaletteRow[];
  virtualizer: Virtualizer<HTMLDivElement, Element>;
  onSelect: (row: ItemRow) => void;
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
export const usePaletteNavigation = ({ rows, virtualizer, onSelect }: Options) => {
  const [activeIndex, setActiveIndex] = useState(-1);

  // New result set → highlight the first item and scroll back to the top
  // (offset 0 keeps the leading group header visible).
  useEffect(() => {
    setActiveIndex(findItem(rows, 0, 1));
    virtualizer.scrollToOffset(0);
  }, [rows, virtualizer]);

  const move = useCallback(
    (dir: 1 | -1) => {
      const start = activeIndex < 0 ? (dir === 1 ? -1 : rows.length) : activeIndex;
      const next = findItem(rows, start + dir, dir);
      if (next === -1) return;
      setActiveIndex(next);
      // Reaching the first item: scroll to the top so its header stays visible.
      if (next === findItem(rows, 0, 1)) virtualizer.scrollToOffset(0);
      else virtualizer.scrollToIndex(next, { align: 'auto' });
    },
    [activeIndex, rows, virtualizer],
  );

  const onKeyDown = useCallback(
    (event: KeyboardEvent) => {
      if (event.key === 'ArrowDown') {
        event.preventDefault();
        move(1);
      } else if (event.key === 'ArrowUp') {
        event.preventDefault();
        move(-1);
      } else if (event.key === 'Enter') {
        const row = rows[activeIndex];
        if (row?.kind === 'item') {
          event.preventDefault();
          onSelect(row);
        }
      }
    },
    [move, rows, activeIndex, onSelect],
  );

  return { activeIndex, setActiveIndex, onKeyDown };
};
