import { Moon02Icon, Sun03Icon } from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { SegmentedControl } from '@/components/ui/segmented-control/segmented-control';
import type { Theme } from '@/stores/ui-store';
import { SettingRow } from '../setting-row';
import { useAppearanceSection } from './use-appearance-section';

const THEME_OPTIONS = [
  {
    value: 'dark' as const,
    label: (
      <>
        <HugeiconsIcon icon={Moon02Icon} className="size-3.5" />
        Escuro
      </>
    ),
  },
  {
    value: 'light' as const,
    label: (
      <>
        <HugeiconsIcon icon={Sun03Icon} className="size-3.5" />
        Claro
      </>
    ),
  },
];

export function AppearanceSection() {
  const { theme, setTheme } = useAppearanceSection();

  return (
    <SettingRow label="Tema">
      <SegmentedControl<Theme>
        size="sm"
        aria-label="Tema"
        value={theme}
        onValueChange={setTheme}
        options={THEME_OPTIONS}
      />
    </SettingRow>
  );
}
