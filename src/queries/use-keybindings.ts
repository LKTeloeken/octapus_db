import { useCallback, useEffect } from 'react';
import {
  overridesFrom,
  type ResolvedBindings,
} from '@/lib/keybindings/resolve';
import { useKeybindingsStore } from '@/stores/keybindings-store';
import { useSettings, useUpdateSettings } from './use-settings';

/**
 * Leva o `keybindings` das preferências (SQLite) ao store dos atalhos, que é
 * de onde os listeners leem. Montado uma vez, no shell do app.
 */
export function useSyncKeybindings() {
  const overrides = useSettings().data?.keybindings;
  const setOverrides = useKeybindingsStore(state => state.setOverrides);

  useEffect(() => setOverrides(overrides), [overrides, setOverrides]);
}

/**
 * Grava as teclas das ações dadas (as outras ficam como estão). Nas
 * preferências vai só o que difere do padrão.
 */
export function useSaveKeybindings() {
  const { mutate, isPending } = useUpdateSettings();

  const save = useCallback(
    (
      changes: Partial<ResolvedBindings>,
      options?: Parameters<typeof mutate>[1],
    ) => {
      const { overrides } = useKeybindingsStore.getState();
      mutate({ keybindings: overridesFrom(changes, overrides) }, options);
    },
    [mutate],
  );

  /** Tudo de volta ao padrão */
  const resetAll = useCallback(
    (options?: Parameters<typeof mutate>[1]) =>
      mutate({ keybindings: {} }, options),
    [mutate],
  );

  return { save, resetAll, isSaving: isPending };
}
