import { useEffect, useRef, useState } from 'react';
import { isTypingTarget, useShortcuts } from '@/lib/keybindings/use-shortcuts';
import type { ResultsViewMode } from './results-table.types';

/**
 * O atalho de trocar a visualização (Tab) vale aqui? Só com o foco numa das
 * vistas (`data-results-view`) e fora de um campo (a busca) ou de uma célula
 * em edição — ali ele segue sendo o Tab de sempre.
 */
export const canToggleView = (target: EventTarget | null, editing: boolean) => {
  if (editing) return false;
  const element = target as HTMLElement | null;
  return (
    !!element?.closest?.('[data-results-view]') && !isTypingTarget(element)
  );
};

/**
 * Tabela ↔ Vertical: o estado da visualização e o Tab que alterna. A vista
 * nova recebe o foco, para o próximo Tab (e as setas) continuarem nela.
 */
export const useViewToggle = (editing: boolean) => {
  const [viewMode, setViewMode] = useState<ResultsViewMode>('table');
  const viewsRef = useRef<HTMLDivElement>(null);
  const refocusView = useRef(false);

  const { onKeyDown: onViewKeyDown } = useShortcuts(
    {
      'grid.toggleView': () => {
        refocusView.current = true;
        setViewMode(mode => (mode === 'table' ? 'vertical' : 'table'));
      },
    },
    {
      target: 'element',
      when: event => canToggleView(event.target, editing),
    },
  );

  useEffect(() => {
    if (!refocusView.current) return;
    refocusView.current = false;
    viewsRef.current
      ?.querySelector<HTMLElement>('[data-results-view]')
      ?.focus({ preventScroll: true });
  }, [viewMode]);

  return { viewMode, setViewMode, viewsRef, onViewKeyDown };
};
