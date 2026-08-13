import { create } from 'zustand';
import { persist } from 'zustand/middleware';

export type Theme = 'dark' | 'light';

interface UiState {
  theme: Theme;
  setTheme: (theme: Theme) => void;
}

export const useUiStore = create<UiState>()(
  persist(
    set => ({
      theme: 'dark',
      setTheme: theme => set({ theme }),
    }),
    { name: 'octapus-ui' },
  ),
);
