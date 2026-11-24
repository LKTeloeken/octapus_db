import { memo, useMemo } from 'react';
import type { GridMatches } from '@/lib/grid-search';
import { cn } from '@/lib/utils';
import type { PendingMarker } from '../results-table.types';

interface ResultsTableScrollMarkersProps {
  rowCount: number;
  matches: GridMatches;
  pending: PendingMarker[];
  /** Topo da área rolável das linhas (abaixo do cabeçalho), em px */
  top: number;
}

/** Quantas faixas o trilho tem: dois marcadores na mesma faixa viram um */
const BUCKETS = 240;

const KIND_CLASS: Record<PendingMarker['kind'] | 'match', string> = {
  match: 'bg-match',
  edited: 'bg-warning',
  added: 'bg-success',
  removed: 'bg-danger',
};

/**
 * As ventosas no trilho da scrollbar (DESIGN.md, Scrollbar): onde estão os
 * resultados da busca e as linhas pendentes, na proporção da grade. Não
 * recebe clique — a scrollbar nativa continua por baixo.
 */
export const ResultsTableScrollMarkers = memo(
  ({ rowCount, matches, pending, top }: ResultsTableScrollMarkersProps) => {
    const markers = useMemo(() => {
      if (rowCount === 0) return [];
      const seen = new Map<string, { bucket: number; kind: string }>();
      const put = (row: number, kind: string) => {
        const bucket = Math.min(
          BUCKETS - 1,
          Math.floor((row / rowCount) * BUCKETS),
        );
        const key = `${kind}:${bucket}`;
        if (!seen.has(key)) seen.set(key, { bucket, kind });
      };
      for (let index = 0; index < matches.count; index++) {
        put(matches.data[index * 2], 'match');
      }
      for (const marker of pending) put(marker.row, marker.kind);
      return Array.from(seen.values());
    }, [rowCount, matches, pending]);

    if (markers.length === 0) return null;

    return (
      <div
        aria-hidden
        className="pointer-events-none absolute right-0.5 bottom-3 w-1.5"
        style={{ top: `${top + 4}px` }}
      >
        {markers.map(({ bucket, kind }) => (
          <span
            key={`${kind}:${bucket}`}
            className={cn(
              'absolute left-0 size-1.5 rounded-full opacity-80',
              KIND_CLASS[kind as keyof typeof KIND_CLASS],
            )}
            style={{ top: `${(bucket / BUCKETS) * 100}%` }}
          />
        ))}
      </div>
    );
  },
);

ResultsTableScrollMarkers.displayName = 'ResultsTableScrollMarkers';
