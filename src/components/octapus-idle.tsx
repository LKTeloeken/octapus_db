import { cn } from '@/lib/utils';
import { useUiStore } from '@/stores/ui-store';
import { OCTAPUS_PATH, OCTAPUS_VIEWBOX } from './octapus-thinking/octapus-path';

interface OctapusIdleProps {
  /** Tamanho em classes do Tailwind; a cor vem do `text-*` */
  className?: string;
  /** Rótulo para leitores de tela; sem ele, o polvo é decorativo */
  label?: string;
}

/**
 * O polvo parado (o logo) como mascote — tela vazia e barra de status
 * (DESIGN.md §6). Diferente do `OctapusMark` (a marca da sidebar), este
 * respeita a preferência "Polvo".
 * Parado de propósito: nada em loop quando o app não está fazendo nada.
 * Respeita a preferência "Polvo: Nunca".
 */
export function OctapusIdle({ className, label }: OctapusIdleProps) {
  const mascot = useUiStore(state => state.mascot);
  if (mascot === 'never') return null;
  return (
    <svg
      viewBox={OCTAPUS_VIEWBOX}
      className={cn('size-20 shrink-0 text-tinta-text', className)}
      {...(label
        ? { role: 'img', 'aria-label': label }
        : { 'aria-hidden': true })}
    >
      <path d={OCTAPUS_PATH} fill="currentColor" fillRule="evenodd" />
    </svg>
  );
}
