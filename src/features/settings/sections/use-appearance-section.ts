import { useUiStore } from '@/stores/ui-store';

export const useAppearanceSection = () => {
  const theme = useUiStore(state => state.theme);
  const setTheme = useUiStore(state => state.setTheme);
  return { theme, setTheme };
};
