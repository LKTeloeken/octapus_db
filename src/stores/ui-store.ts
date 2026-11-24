import { useSyncExternalStore } from 'react';
import { create } from 'zustand';
import { persist } from 'zustand/middleware';

/** Os temas do Abissal (DESIGN.md §2) e `system`, que segue o SO entre Abissal e Nácar */
export type Theme = 'abissal' | 'nacar' | 'fossa' | 'recife' | 'system';
/** O acento troca só a tinta (DESIGN.md §2.2) */
export type Accent = 'tinta' | 'coral' | 'biolume' | 'alga' | 'ambar' | 'azul';
export type Density = 'compact' | 'normal' | 'comfy';
export type UiFont = 'geist' | 'inter' | 'system';
export type MonoFont = 'geist' | 'jetbrains' | 'system';
/** Onde o polvo aparece: sempre, só quando o app está ocupado ou nunca */
export type MascotMode = 'always' | 'busy' | 'never';

/** Claro ou escuro, já resolvido: o que o CodeMirror e o `.dark` precisam saber */
export type ColorMode = 'dark' | 'light';

interface UiState {
  theme: Theme;
  accent: Accent;
  density: Density;
  uiFont: UiFont;
  monoFont: MonoFont;
  mascot: MascotMode;
  /** Listras alternadas na grade */
  zebra: boolean;
  setTheme: (theme: Theme) => void;
  setAccent: (accent: Accent) => void;
  setDensity: (density: Density) => void;
  setUiFont: (font: UiFont) => void;
  setMonoFont: (font: MonoFont) => void;
  setMascot: (mode: MascotMode) => void;
  setZebra: (zebra: boolean) => void;
}

export const useUiStore = create<UiState>()(
  persist(
    set => ({
      theme: 'abissal',
      accent: 'tinta',
      density: 'normal',
      uiFont: 'geist',
      monoFont: 'geist',
      mascot: 'busy',
      zebra: false,
      setTheme: theme => set({ theme }),
      setAccent: accent => set({ accent }),
      setDensity: density => set({ density }),
      setUiFont: uiFont => set({ uiFont }),
      setMonoFont: monoFont => set({ monoFont }),
      setMascot: mascot => set({ mascot }),
      setZebra: zebra => set({ zebra }),
    }),
    {
      name: 'octapus-ui',
      version: 1,
      // v0 (Ink) guardava só 'dark' | 'light'
      migrate: (persisted, version) => {
        const state = (persisted ?? {}) as Record<string, unknown>;
        if (version < 1) {
          state.theme = state.theme === 'light' ? 'nacar' : 'abissal';
        }
        return state as unknown as UiState;
      },
    },
  ),
);

const DARK_QUERY = '(prefers-color-scheme: dark)';

function subscribeSystem(callback: () => void) {
  if (typeof window === 'undefined' || !window.matchMedia) return () => {};
  const media = window.matchMedia(DARK_QUERY);
  media.addEventListener('change', callback);
  return () => media.removeEventListener('change', callback);
}

function systemIsDark() {
  if (typeof window === 'undefined' || !window.matchMedia) return true;
  return window.matchMedia(DARK_QUERY).matches;
}

/** O modo concreto de um tema, dado o que o SO diz agora */
export function resolveColorMode(theme: Theme, systemDark: boolean): ColorMode {
  if (theme === 'system') return systemDark ? 'dark' : 'light';
  return theme === 'nacar' ? 'light' : 'dark';
}

/** Claro/escuro do tema atual, acompanhando a troca do SO com o app aberto */
export function useColorMode(): ColorMode {
  const theme = useUiStore(state => state.theme);
  const systemDark = useSyncExternalStore(
    subscribeSystem,
    systemIsDark,
    () => true,
  );
  return resolveColorMode(theme, systemDark);
}

/** Alturas que dependem da densidade (as mesmas do `--row-h` no globals.css) */
export const DENSITY_ROWS: Record<Density, { row: number; tree: number }> = {
  compact: { row: 24, tree: 24 },
  normal: { row: 28, tree: 28 },
  comfy: { row: 32, tree: 32 },
};

export function useDensityRows() {
  const density = useUiStore(state => state.density);
  return DENSITY_ROWS[density];
}
