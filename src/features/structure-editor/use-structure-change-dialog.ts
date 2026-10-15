import toast from 'react-hot-toast';
import type { StructureChange } from '@/api/types/structure.types';
import { useDebouncedValue } from '@/lib/use-debounced-value';
import {
  useApplyStructureChange,
  useStructureChangePreview,
} from '@/queries/use-structure-change';
import {
  useStructureEditorStore,
  type StructureTarget,
} from '@/stores/structure-editor-store';

/** Quanto esperar a digitação parar antes de pedir a prévia ao backend */
const PREVIEW_DEBOUNCE_MS = 250;

interface Options {
  /** Mensagem do toast quando der certo */
  success: string;
  /** Depois de aplicar e antes de fechar (expandir o schema, fechar a aba) */
  onApplied?: () => void;
}

/**
 * O miolo de todo diálogo de estrutura: a prévia do SQL (com debounce, pelo
 * backend) e o aplicar. `change` null = formulário incompleto — sem prévia e
 * sem aplicar.
 */
export function useStructureChangeDialog(
  target: StructureTarget,
  change: StructureChange | null,
  { success, onApplied }: Options,
) {
  const close = useStructureEditorStore(state => state.close);
  const debounced = useDebouncedValue(change, PREVIEW_DEBOUNCE_MS);
  const preview = useStructureChangePreview(
    target.serverId,
    target.database,
    debounced,
  );
  const apply = useApplyStructureChange();

  // A prévia na tela é de uma versão anterior do formulário
  const catchingUp = JSON.stringify(debounced) !== JSON.stringify(change);
  const statements = change ? preview.data : undefined;
  const error = change && preview.isError ? preview.error.message : null;

  const canApply =
    change !== null &&
    !catchingUp &&
    !preview.isFetching &&
    !error &&
    !!statements?.length &&
    !apply.isPending;

  const submit = async () => {
    if (!change || !canApply) return;
    try {
      await apply.mutateAsync({
        serverId: target.serverId,
        database: target.database,
        change,
      });
      toast.success(success);
      onApplied?.();
      close();
    } catch (failure) {
      toast.error(failure instanceof Error ? failure.message : String(failure));
    }
  };

  return {
    preview: {
      statements,
      error,
      isLoading: change !== null && (catchingUp || preview.isFetching),
    },
    canApply,
    isApplying: apply.isPending,
    submit,
    close,
  };
}
