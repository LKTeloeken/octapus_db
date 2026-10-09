import type { SchemaDrift, ShapeGroup } from '@/api/types/catalog.types';
import { formatCount } from '@/lib/format-count';

/** Quantos nomes de relação o `title` de uma variação lista */
const NAMES_IN_TITLE = 10;

const plural = (count: number, one: string, many: string) =>
  `${formatCount(count)} ${count === 1 ? one : many}`;

/** `−3`, `+1` ou `−3 +1` (sinal de menos de verdade, não hífen) */
function diffLabel(missing: number, extra: number): string {
  return [missing > 0 && `−${missing}`, extra > 0 && `+${extra}`]
    .filter(Boolean)
    .join(' ');
}

function listNames(names: string[]): string {
  const shown = names.slice(0, NAMES_IN_TITLE).join(', ');
  const rest = names.length - NAMES_IN_TITLE;
  return rest > 0 ? `${shown} e mais ${formatCount(rest)}` : shown;
}

/** Aviso na linha de um schema fora do formato principal */
export function driftBadge(
  drift: SchemaDrift | null,
): { label: string; title: string } | undefined {
  if (!drift || (drift.missing === 0 && drift.extra === 0)) return undefined;
  const parts = [
    drift.missing > 0 && `faltam ${plural(drift.missing, 'tabela', 'tabelas')}`,
    drift.extra > 0 && `sobram ${plural(drift.extra, 'tabela', 'tabelas')}`,
  ].filter(Boolean);
  return {
    label: diffLabel(drift.missing, drift.extra),
    title: `Fora do formato principal: ${parts.join(' e ')}`,
  };
}

/**
 * Rótulo, contagem e aviso de um grupo da árvore agrupada por formato. Nomes
 * curtos de propósito: a sidebar é estreita e a contagem de schemas não pode
 * sumir; o detalhe (quantas tabelas, quais faltam) vai no `title` do aviso.
 */
export function shapeGroupRow(group: ShapeGroup): {
  name: string;
  subLabel: string;
  badge?: { label: string; title: string };
} {
  const subLabel = formatCount(group.schemas);
  switch (group.role) {
    case 'dominant':
      return { name: 'Formato principal', subLabel };
    case 'variant': {
      const lines = [
        `Variação com ${plural(group.tables ?? 0, 'tabela', 'tabelas')}`,
        group.missing.length > 0 && `Faltam: ${listNames(group.missing)}`,
        group.extra.length > 0 && `Sobram: ${listNames(group.extra)}`,
      ].filter(Boolean);
      return {
        name: 'Variação',
        subLabel,
        badge: {
          label: diffLabel(group.missing.length, group.extra.length),
          title: lines.join('\n'),
        },
      };
    }
    case 'other':
      return { name: 'Outros schemas', subLabel };
  }
}
