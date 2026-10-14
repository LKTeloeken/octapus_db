import { memo } from 'react';
import type { GridMatcher } from '@/lib/grid-search';
import { cn } from '@/lib/utils';

interface HighlightedTextProps {
  text: string;
  matcher: GridMatcher;
  /** Célula do resultado atual da navegação: realce mais forte + contorno */
  isCurrent: boolean;
}

/**
 * Texto da célula com os trechos da busca em `<mark>` (DESIGN.md §2.4). Só
 * as células desenhadas pelo virtualizador chegam aqui, e o `ranges` já vem
 * limitado em caracteres e em trechos — o custo fica no que está na tela.
 */
export const HighlightedText = memo(
  ({ text, matcher, isCurrent }: HighlightedTextProps) => {
    const ranges = matcher.ranges(text);
    if (ranges.length === 0) return <>{text}</>;

    const parts: React.ReactNode[] = [];
    let cursor = 0;
    ranges.forEach(([start, end], i) => {
      if (start > cursor) parts.push(text.slice(cursor, start));
      parts.push(
        <mark
          key={i}
          className={cn(
            'rounded-xs text-fg',
            isCurrent
              ? 'bg-match-strong shadow-[inset_0_0_0_1px_var(--match)]'
              : 'bg-match-soft',
          )}
        >
          {text.slice(start, end)}
        </mark>,
      );
      cursor = end;
    });
    if (cursor < text.length) parts.push(text.slice(cursor));

    return <>{parts}</>;
  },
);

HighlightedText.displayName = 'HighlightedText';
