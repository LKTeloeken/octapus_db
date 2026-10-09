import { Download04Icon, PaintBoardIcon } from '@hugeicons/core-free-icons';
import { AppearanceSection } from './sections/appearance-section';
import { UpdatesSection } from './sections/updates-section';
import type { SettingsSection } from './settings-dialog.types';

/**
 * Seções do diálogo, na ordem do menu. Uma configuração nova entra numa
 * seção existente (uma `SettingRow` a mais) ou ganha a sua aqui. O valor mora
 * nas preferências do SQLite (`useSettings`); o tema é a exceção — fica no
 * `ui-store` (localStorage), lido antes da primeira pintura para não piscar.
 */
export const SETTINGS_SECTIONS: SettingsSection[] = [
  {
    id: 'appearance',
    label: 'Aparência',
    description: 'Tema e visual do app.',
    icon: PaintBoardIcon,
    Component: AppearanceSection,
  },
  {
    id: 'updates',
    label: 'Atualizações',
    description: 'Como o Octapus procura versões novas.',
    icon: Download04Icon,
    Component: UpdatesSection,
  },
];
