import { useEffect, useRef } from 'react';
import type { QueryColumnInfo } from '@/api/types/query.types';

/**
 * Limpa as colunas ocultas quando o conjunto de colunas realmente muda (outra
 * query/tabela). Ignora a transição de carregamento (`[]` → colunas) para não
 * apagar o estado salvo da aba ao remontar, e o refetch normal mantém os nomes,
 * então não dispara. Trocar de aba também não: entre duas abas do mesmo tipo o
 * componente é reaproveitado e as colunas "mudam" para as da outra aba, que
 * tem as próprias colunas ocultas.
 */
export function useHiddenColumnsReset(
  /** Dono das colunas ocultas: o id da aba */
  scopeId: string,
  columns: QueryColumnInfo[],
  hiddenCount: number,
  reset: () => void,
): void {
  // NUL como separador: não colide com nomes de coluna (que podem ter espaço).
  const columnSignature = columns.map(c => c.name).join('\u0000');
  const prevRef = useRef({ scopeId, signature: columnSignature });

  useEffect(() => {
    const prev = prevRef.current;
    if (prev.scopeId === scopeId && prev.signature === columnSignature) return;
    prevRef.current = { scopeId, signature: columnSignature };
    // Outra aba na mesma instância: é como montar de novo.
    if (prev.scopeId !== scopeId) return;
    // '' → colunas é só o primeiro load depois de montar; não é troca de query.
    if (prev.signature === '' || columnSignature === '') return;
    if (hiddenCount > 0) reset();
  }, [scopeId, columnSignature, hiddenCount, reset]);
}
