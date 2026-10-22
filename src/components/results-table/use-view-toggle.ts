import { useCallback, useEffect, useRef, useState } from 'react';
import type { KeyboardEvent } from 'react';
import type { ResultsViewMode } from './results-table.types';

/** O que a decisão do Tab precisa do evento (o `KeyboardEvent` do React serve) */
type ToggleKeyEvent = Pick<
  KeyboardEvent,
  'key' | 'shiftKey' | 'metaKey' | 'ctrlKey' | 'altKey'
> & { target: EventTarget | null };

/**
 * O Tab troca a visualização? Só o Tab sozinho, com o foco numa das vistas
 * (`data-results-view`) e fora de um campo (a busca) ou de uma célula em
 * edição — ali ele segue sendo o Tab de sempre.
 */
export const isViewToggle = (event: ToggleKeyEvent, editing: boolean) => {
  if (event.key !== 'Tab' || editing) return false;
  if (event.shiftKey || event.metaKey || event.ctrlKey || event.altKey)
    return false;
  const target = event.target as HTMLElement | null;
  if (!target?.closest?.('[data-results-view]')) return false;
  return !(
    target.tagName === 'INPUT' ||
    target.tagName === 'TEXTAREA' ||
    target.isContentEditable
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

  const onViewKeyDown = useCallback(
    (event: KeyboardEvent) => {
      if (!isViewToggle(event, editing)) return;
      event.preventDefault();
      refocusView.current = true;
      setViewMode(mode => (mode === 'table' ? 'vertical' : 'table'));
    },
    [editing],
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
