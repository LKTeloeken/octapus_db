import { useUiStore } from '@/stores/ui-store';

export const useAppearanceSection = () => {
  const theme = useUiStore(state => state.theme);
  const accent = useUiStore(state => state.accent);
  const density = useUiStore(state => state.density);
  const uiFont = useUiStore(state => state.uiFont);
  const monoFont = useUiStore(state => state.monoFont);
  const mascot = useUiStore(state => state.mascot);
  const zebra = useUiStore(state => state.zebra);
  const setTheme = useUiStore(state => state.setTheme);
  const setAccent = useUiStore(state => state.setAccent);
  const setDensity = useUiStore(state => state.setDensity);
  const setUiFont = useUiStore(state => state.setUiFont);
  const setMonoFont = useUiStore(state => state.setMonoFont);
  const setMascot = useUiStore(state => state.setMascot);
  const setZebra = useUiStore(state => state.setZebra);
  return {
    theme,
    accent,
    density,
    uiFont,
    monoFont,
    mascot,
    zebra,
    setTheme,
    setAccent,
    setDensity,
    setUiFont,
    setMonoFont,
    setMascot,
    setZebra,
  };
};
