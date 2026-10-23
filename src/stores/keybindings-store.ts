import { create } from 'zustand';
import {
  resolveBindings,
  type ResolvedBindings,
  type ShortcutOverrides,
} from '@/lib/keybindings/resolve';

interface KeybindingsState {
  /** O que o usuário trocou (o `keybindings` das preferências) */
  overrides: ShortcutOverrides;
  /** Padrões + trocas: as teclas que valem agora para cada ação */
  resolved: ResolvedBindings;
  /**
   * A tela de Atalhos está gravando uma tecla: nenhum atalho dispara, senão
   * gravar o ⌘K abriria a paleta.
   */
  recording: boolean;
  setOverrides: (overrides: ShortcutOverrides | undefined) => void;
  setRecording: (recording: boolean) => void;
}

/**
 * Os atalhos em vigor. Mora num store (e não só no React Query) porque os
 * listeners leem as teclas na hora do evento, com `getState()` — trocar um
 * atalho não precisa religar listener nenhum. Quem sincroniza com as
 * preferências é o `useSyncKeybindings` (`queries/use-keybindings.ts`);
 * antes de elas carregarem valem os padrões.
 */
export const useKeybindingsStore = create<KeybindingsState>(set => ({
  overrides: {},
  resolved: resolveBindings(undefined),
  recording: false,
  setOverrides: overrides =>
    set({
      overrides: overrides ?? {},
      resolved: resolveBindings(overrides),
    }),
  setRecording: recording => set({ recording }),
}));
