import { useCallback, useState } from 'react';
import type { QueryColumnInfo } from '@/api/types/query.types';
import { usePendingChangesStore } from '@/stores/pending-changes-store';
import type { PendingChangesDialogProps } from './pending-changes-dialog';
import {
  buildSaveRowChanges,
  countPendingChanges,
  pendingCountLabel,
} from './pending-changes';
import type { SaveRowChanges } from './results-table.types';

/** O que troca os dados da grade (e por isso perderia as pendências) */
export type GuardedAction = 'sort' | 'filter' | 'reload' | 'run';

const ACTION_LABEL: Record<GuardedAction, string> = {
  sort: 'Ordenar',
  filter: 'Aplicar o filtro',
  reload: 'Recarregar',
  run: 'Executar a consulta',
};

const describe = ({ action, pendingCount: count }: GuardRequest) =>
  `A grade tem ${pendingCountLabel(count)} que não ` +
  `${count === 1 ? 'foi salva' : 'foram salvas'} no banco. ` +
  `${ACTION_LABEL[action]} recarrega os dados: salve antes ou descarte ` +
  `${count === 1 ? 'a alteração' : 'as alterações'}.`;

interface GuardRequest {
  action: GuardedAction;
  run: () => void;
  pendingCount: number;
  /** Falso durante a animação de saída do diálogo */
  open: boolean;
}

/**
 * Ordenar, filtrar, recarregar ou reexecutar troca os dados da grade, e a
 * grade nova invalida as pendências. Com algo pendente, `guard` segura a ação
 * e abre o diálogo: Salvar (grava pelo `onSave` da grade e aplica), Descartar
 * (descarta e aplica) ou Cancelar (não aplica nada; as pendências ficam). Sem
 * pendências, aplica na hora.
 */
export function usePendingChangesGuard(
  /** Dono das pendências: o id da aba */
  scopeId: string,
  columns: QueryColumnInfo[],
  /** O mesmo `onSave` da grade: rejeita quando a gravação falha */
  onSave: (changes: SaveRowChanges) => void | Promise<void>,
) {
  const [request, setRequest] = useState<GuardRequest | null>(null);
  const [isSaving, setIsSaving] = useState(false);
  const clearPending = usePendingChangesStore(state => state.clear);

  const guard = useCallback(
    (action: GuardedAction, run: () => void) => {
      const pending = usePendingChangesStore.getState().byScope.get(scopeId);
      if (!pending) {
        run();
        return;
      }
      setRequest({
        action,
        run,
        pendingCount: countPendingChanges(pending),
        open: true,
      });
    },
    [scopeId],
  );

  // Fecha mantendo o resto, para o diálogo sair animado com o texto no lugar
  const close = useCallback(
    () => setRequest(prev => prev && { ...prev, open: false }),
    [],
  );

  const discard = useCallback(() => {
    if (!request?.open) return;
    clearPending(scopeId);
    close();
    request.run();
  }, [request, clearPending, scopeId, close]);

  const save = useCallback(async () => {
    if (!request?.open) return;
    const pending = usePendingChangesStore.getState().byScope.get(scopeId);
    if (pending) {
      setIsSaving(true);
      try {
        await onSave(buildSaveRowChanges(pending, columns));
      } catch {
        // O `onSave` já avisou do erro; as pendências ficam e nada é aplicado
        close();
        return;
      } finally {
        setIsSaving(false);
      }
      clearPending(scopeId);
    }
    close();
    request.run();
  }, [request, scopeId, onSave, columns, clearPending, close]);

  const dialogProps: PendingChangesDialogProps = {
    open: request?.open ?? false,
    title: 'Salvar as alterações pendentes?',
    description: request ? describe(request) : '',
    discardLabel: 'Descartar',
    onDiscard: discard,
    onCancel: close,
    onSave: () => void save(),
    isSaving,
  };

  return { guard, dialogProps };
}
