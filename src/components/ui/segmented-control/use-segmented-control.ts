import { useCallback, useLayoutEffect, useRef, useState } from 'react';
import type { KeyboardEvent } from 'react';
import type {
  SegmentedControlOption,
  ThumbRect,
} from './segmented-control.types';

/**
 * Posição do indicador (medida do botão ativo) e navegação por setas no
 * padrão de radiogroup: as setas trocam o valor e levam o foco junto.
 */
export const useSegmentedControl = <T extends string>({
  value,
  onValueChange,
  options,
}: {
  value: T;
  onValueChange: (value: T) => void;
  options: SegmentedControlOption<T>[];
}) => {
  const itemRefs = useRef(new Map<T, HTMLButtonElement>());
  const [thumb, setThumb] = useState<ThumbRect | null>(null);
  // A primeira medida posiciona sem animar: o indicador não "voa" ao montar.
  const [isReady, setIsReady] = useState(false);

  const setItemRef = useCallback(
    (optionValue: T) => (node: HTMLButtonElement | null) => {
      if (node) itemRefs.current.set(optionValue, node);
      else itemRefs.current.delete(optionValue);
    },
    [],
  );

  // Identidade estável das opções: o array em si costuma ser recriado a cada render.
  const optionsKey = options.map(option => option.value).join('\u0000');

  useLayoutEffect(() => {
    // Só troca o estado quando a posição muda: `options` costuma chegar como
    // array novo a cada render, e um objeto novo aqui viraria um loop.
    const measure = () => {
      const node = itemRefs.current.get(value);
      if (!node) return;
      const left = node.offsetLeft;
      const width = node.offsetWidth;
      setThumb(prev =>
        prev && prev.left === left && prev.width === width
          ? prev
          : { left, width },
      );
    };

    measure();
    const frame = requestAnimationFrame(() => setIsReady(true));

    // Troca de fonte ou de rótulo muda a largura dos itens.
    const observer = new ResizeObserver(measure);
    itemRefs.current.forEach(node => observer.observe(node));

    return () => {
      cancelAnimationFrame(frame);
      observer.disconnect();
    };
  }, [value, optionsKey]);

  const onKeyDown = useCallback(
    (event: KeyboardEvent<HTMLDivElement>) => {
      const step =
        event.key === 'ArrowRight' || event.key === 'ArrowDown'
          ? 1
          : event.key === 'ArrowLeft' || event.key === 'ArrowUp'
            ? -1
            : 0;
      if (step === 0) return;

      event.preventDefault();
      const index = options.findIndex(option => option.value === value);
      const next = options[(index + step + options.length) % options.length];
      onValueChange(next.value);
      itemRefs.current.get(next.value)?.focus();
    },
    [options, value, onValueChange],
  );

  return { thumb, isReady, setItemRef, onKeyDown };
};
