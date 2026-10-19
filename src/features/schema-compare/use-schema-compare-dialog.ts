import { useQuery } from '@tanstack/react-query';
import { useState } from 'react';
import { useDebouncedValue } from '@/lib/use-debounced-value';
import {
  catalogChildrenQuery,
  useSchemaComparison,
} from '@/queries/use-catalog';
import {
  useStructureEditorStore,
  type StructureDialog,
} from '@/stores/structure-editor-store';

export type CompareDialog = Extract<
  StructureDialog,
  { kind: 'compareSchemas' }
>;

/** Quantas sugestões cada campo mostra (o catálogo filtra; milhares de schemas) */
const SUGGESTIONS = 50;

/** Sugestões de schema para um campo, filtradas pelo que já foi digitado */
function useSchemaSuggestions(
  serverId: number,
  database: string,
  text: string,
) {
  const filter = useDebouncedValue(text.trim(), 150);
  const query = useQuery(
    catalogChildrenQuery(
      serverId,
      database,
      { kind: 'schemas' },
      filter,
      SUGGESTIONS,
    ),
  );
  return query.data?.items.map(node => node.name) ?? [];
}

/** Dois schemas, base e outro: compara ao pedir (botão ou Enter) */
export function useSchemaCompareDialog({
  serverId,
  database,
  base: initialBase,
}: CompareDialog) {
  const close = useStructureEditorStore(state => state.close);
  const [base, setBase] = useState(initialBase ?? '');
  const [other, setOther] = useState('');
  const [pair, setPair] = useState<{ base: string; other: string } | null>(
    null,
  );

  const baseSuggestions = useSchemaSuggestions(serverId, database, base);
  const otherSuggestions = useSchemaSuggestions(serverId, database, other);
  const query = useSchemaComparison(pair && { serverId, database, ...pair });

  const canCompare =
    !!base.trim() && !!other.trim() && base.trim() !== other.trim();

  const compare = () => {
    if (!canCompare) return;
    const next = { base: base.trim(), other: other.trim() };
    // O mesmo par de novo refaz (os schemas podem ter mudado)
    if (pair?.base === next.base && pair.other === next.other)
      void query.refetch();
    else setPair(next);
  };

  const swap = () => {
    setBase(other);
    setOther(base);
  };

  return {
    base,
    setBase,
    other,
    setOther,
    baseSuggestions,
    otherSuggestions,
    swap,
    canCompare,
    compare,
    result: query.data,
    isComparing: query.isFetching,
    error: query.isError ? query.error.message : null,
    close,
  };
}
