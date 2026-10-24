import { useEffect, useRef } from 'react';
import type { ImperativePanelHandle } from 'react-resizable-panels';
import { useWindowWidth } from './window-size';

/**
 * O `react-resizable-panels` (v2) só aceita tamanhos em % do grupo. Os painéis
 * laterais precisam de limites em px: em % a barra lateral virava ~160 px numa
 * janela de 800 px e a árvore perdia os nomes. Estes helpers convertem px em %
 * da largura atual do grupo.
 */

/** Moldura do grupo principal (`p-2` do app.tsx): 8 px de cada lado */
const GROUP_FRAME_PX = 16;

/**
 * Quanto `px` representa, em %, de um grupo de `groupWidth` px — limitado a
 * `maxPercent`, para que o mínimo nunca passe do máximo numa janela estreita.
 */
export function pxToPercent(
  px: number,
  groupWidth: number,
  maxPercent = 100,
): number {
  if (groupWidth <= 0) return maxPercent;
  return Math.min(maxPercent, (px / groupWidth) * 100);
}

/** Largura do grupo principal de painéis, acompanhando o redimensionamento da janela */
export const useMainGroupWidth = (): number =>
  useWindowWidth() - GROUP_FRAME_PX;

interface PixelPanelOptions {
  /** Largura inicial */
  defaultPx: number;
  /** Largura mínima (cede ao `maxPercent` numa janela estreita) */
  minPx: number;
  maxPercent: number;
}

/**
 * Painel lateral com largura em px: guarda a largura escolhida ao arrastar e a
 * mantém quando a janela muda de tamanho — em % ele cresceria junto com a
 * janela. Espalhe `panelProps` no `ResizablePanel` e passe `onDragging` à alça
 * vizinha.
 */
export function usePixelPanel(
  groupWidth: number,
  { defaultPx, minPx, maxPercent }: PixelPanelOptions,
) {
  const ref = useRef<ImperativePanelHandle>(null);
  const widthPx = useRef(defaultPx);
  const minSize = pxToPercent(minPx, groupWidth, maxPercent);

  useEffect(() => {
    ref.current?.resize(
      Math.max(minSize, pxToPercent(widthPx.current, groupWidth, maxPercent)),
    );
  }, [groupWidth, minSize, maxPercent]);

  const onDragging = (dragging: boolean) => {
    if (!dragging && ref.current) {
      widthPx.current = (ref.current.getSize() * groupWidth) / 100;
    }
  };

  return {
    panelProps: {
      ref,
      defaultSize: Math.max(
        minSize,
        pxToPercent(widthPx.current, groupWidth, maxPercent),
      ),
      minSize,
      maxSize: maxPercent,
    },
    onDragging,
  };
}
