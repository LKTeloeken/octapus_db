import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import { SegmentedControl } from '@/components/ui/segmented-control/segmented-control';
import { Switch } from '@/components/ui/switch';
import { cn } from '@/lib/utils';
import type {
  Accent,
  Density,
  MascotMode,
  MonoFont,
  Theme,
  UiFont,
} from '@/stores/ui-store';
import { SettingRow } from '../setting-row';
import { useAppearanceSection } from './use-appearance-section';

/**
 * Amostras de tema e acento: cada uma pinta com os tokens do próprio tema
 * (`data-theme`/`data-accent` num wrapper), então nada de cor crua aqui.
 */
const THEMES: { value: Theme; label: string; scope: string; dark: boolean }[] =
  [
    { value: 'abissal', label: 'Abissal', scope: '', dark: true },
    { value: 'nacar', label: 'Nácar', scope: '', dark: false },
    { value: 'fossa', label: 'Fossa', scope: 'fossa', dark: true },
    { value: 'recife', label: 'Recife', scope: 'recife', dark: true },
  ];

const ACCENTS: { value: Accent; label: string }[] = [
  { value: 'tinta', label: 'Tinta' },
  { value: 'coral', label: 'Coral' },
  { value: 'biolume', label: 'Biolume' },
  { value: 'alga', label: 'Alga' },
  { value: 'ambar', label: 'Âmbar' },
  { value: 'azul', label: 'Azul' },
];

const DENSITY_OPTIONS: { value: Density; label: string }[] = [
  { value: 'compact', label: 'Compacta' },
  { value: 'normal', label: 'Normal' },
  { value: 'comfy', label: 'Confortável' },
];

const MASCOT_OPTIONS: { value: MascotMode; label: string }[] = [
  { value: 'always', label: 'Sempre' },
  { value: 'busy', label: 'Quando ocupado' },
  { value: 'never', label: 'Nunca' },
];

const UI_FONTS: { value: UiFont; label: string }[] = [
  { value: 'geist', label: 'Geist' },
  { value: 'inter', label: 'Inter' },
  { value: 'system', label: 'Do sistema' },
];

const MONO_FONTS: { value: MonoFont; label: string }[] = [
  { value: 'geist', label: 'Geist Mono' },
  { value: 'jetbrains', label: 'JetBrains Mono' },
  { value: 'system', label: 'Do sistema' },
];

function ThemeSwatch({
  option,
  selected,
  onSelect,
}: {
  option: (typeof THEMES)[number];
  selected: boolean;
  onSelect: () => void;
}) {
  return (
    <button
      type="button"
      aria-pressed={selected}
      onClick={onSelect}
      className={cn(
        'flex w-18 cursor-pointer flex-col gap-1 rounded-md p-1 text-small text-fg-muted ring-1 ring-line transition-colors hover:bg-hover focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring',
        selected && 'text-fg ring-2 ring-tinta-text',
      )}
    >
      {/* O wrapper aplica o tema da amostra só a ela */}
      <span
        className={cn(option.dark ? 'dark' : 'light')}
        data-theme={option.scope || undefined}
      >
        <span className="flex h-9 overflow-hidden rounded-sm">
          <span className="flex-1 bg-bg" />
          <span className="flex-1 bg-surface-1" />
          <span className="flex-1 bg-tinta" />
        </span>
      </span>
      {option.label}
    </button>
  );
}

export function AppearanceSection() {
  const appearance = useAppearanceSection();

  return (
    <div>
      <SettingRow label="Tema" description="Ou siga o claro/escuro do sistema.">
        <div className="flex flex-col items-end gap-2">
          <div className="flex gap-2" role="group" aria-label="Tema">
            {THEMES.map(option => (
              <ThemeSwatch
                key={option.value}
                option={option}
                selected={appearance.theme === option.value}
                onSelect={() => appearance.setTheme(option.value)}
              />
            ))}
          </div>
          <button
            type="button"
            aria-pressed={appearance.theme === 'system'}
            onClick={() => appearance.setTheme('system')}
            className={cn(
              'cursor-pointer rounded-sm px-2 py-0.5 text-small text-fg-muted transition-colors hover:bg-hover hover:text-fg',
              appearance.theme === 'system' && 'bg-active text-fg',
            )}
          >
            Seguir o sistema
          </button>
        </div>
      </SettingRow>

      <SettingRow
        label="Acento"
        description="A cor das ações principais, da seleção e do foco."
      >
        <div className="flex gap-2" role="radiogroup" aria-label="Acento">
          {ACCENTS.map(option => {
            const selected = appearance.accent === option.value;
            return (
              <button
                key={option.value}
                type="button"
                role="radio"
                aria-checked={selected}
                aria-label={option.label}
                title={option.label}
                onClick={() => appearance.setAccent(option.value)}
                data-accent={option.value}
                className={cn(
                  'size-5 cursor-pointer rounded-full bg-tinta transition-transform hover:scale-110 focus-visible:outline-2 focus-visible:outline-offset-2 focus-visible:outline-ring',
                  selected &&
                    'ring-2 ring-fg ring-offset-2 ring-offset-surface-1',
                )}
              />
            );
          })}
        </div>
      </SettingRow>

      <SettingRow
        label="Densidade"
        description="Altura das linhas da grade, da árvore e dos menus."
      >
        <SegmentedControl<Density>
          size="sm"
          aria-label="Densidade"
          value={appearance.density}
          onValueChange={appearance.setDensity}
          options={DENSITY_OPTIONS}
        />
      </SettingRow>

      <SettingRow label="Fonte da interface">
        <Select
          value={appearance.uiFont}
          onValueChange={value => appearance.setUiFont(value as UiFont)}
        >
          <SelectTrigger
            size="sm"
            aria-label="Fonte da interface"
            className="w-40"
          >
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {UI_FONTS.map(font => (
              <SelectItem key={font.value} value={font.value}>
                {font.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </SettingRow>

      <SettingRow
        label="Fonte dos dados"
        description="Grade, editor SQL e atalhos."
      >
        <Select
          value={appearance.monoFont}
          onValueChange={value => appearance.setMonoFont(value as MonoFont)}
        >
          <SelectTrigger
            size="sm"
            aria-label="Fonte dos dados"
            className="w-40 font-mono"
          >
            <SelectValue />
          </SelectTrigger>
          <SelectContent>
            {MONO_FONTS.map(font => (
              <SelectItem key={font.value} value={font.value}>
                {font.label}
              </SelectItem>
            ))}
          </SelectContent>
        </Select>
      </SettingRow>

      <SettingRow
        htmlFor="appearance-zebra"
        label="Listras na grade"
        description="Linhas alternadas com um fundo um pouco diferente."
      >
        <Switch
          id="appearance-zebra"
          checked={appearance.zebra}
          onCheckedChange={appearance.setZebra}
        />
      </SettingRow>

      <SettingRow
        label="Polvo"
        description="Na barra de status e nas telas vazias. Quando ocupado: só enquanto algo roda."
      >
        <SegmentedControl<MascotMode>
          size="sm"
          aria-label="Polvo"
          value={appearance.mascot}
          onValueChange={appearance.setMascot}
          options={MASCOT_OPTIONS}
        />
      </SettingRow>
    </div>
  );
}
