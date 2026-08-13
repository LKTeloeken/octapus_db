import { useEffect, useRef } from 'react';
import type { QueryColumnInfo } from '@/api/types/query.types';

/**
 * Limpa as colunas ocultas quando o conjunto de colunas realmente muda (outra
 * query/tabela). Ignora a transição de carregamento (`[]` → colunas) para não
 * apagar o estado salvo da aba ao remontar, e o refetch normal mantém os nomes,
 * então não dispara.
 */
export function useHiddenColumnsReset(
  columns: QueryColumnInfo[],
  hiddenCount: number,
  reset: () => void,
): void {
  // NUL como separador: não colide com nomes de coluna (que podem ter espaço).
  const columnSignature = columns.map(c => c.name).join('\u0000');
  const prevSignatureRef = useRef(columnSignature);

  useEffect(() => {
    const prev = prevSignatureRef.current;
    if (prev === columnSignature) return;
    prevSignatureRef.current = columnSignature;
    // '' → colunas é só o primeiro load depois de montar; não é troca de query.
    if (prev === '' || columnSignature === '') return;
    if (hiddenCount > 0) reset();
  }, [columnSignature, hiddenCount, reset]);
}
