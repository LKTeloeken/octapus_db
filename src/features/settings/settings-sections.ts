import {
  AiChat02Icon,
  Bug01Icon,
  DatabaseIcon,
  Download04Icon,
  KeyboardIcon,
  PaintBoardIcon,
} from '@hugeicons/core-free-icons';
import { AppearanceSection } from './sections/appearance-section';
import { AssistantSection } from './sections/assistant-section';
import { DiagnosticsSection } from './sections/diagnostics-section';
import { PgToolsSection } from './sections/pg-tools-section';
import { ShortcutsSection } from './sections/shortcuts-section';
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
    id: 'shortcuts',
    label: 'Atalhos',
    description:
      'As teclas de cada ação. Uma ação aceita mais de uma tecla — as setas e o hjkl do vim, por exemplo.',
    icon: KeyboardIcon,
    Component: ShortcutsSection,
  },
  {
    id: 'pg-tools',
    label: 'PostgreSQL',
    description: 'Programas do PostgreSQL usados no backup e na restauração.',
    icon: DatabaseIcon,
    Component: PgToolsSection,
  },
  {
    id: 'assistant',
    label: 'Assistente',
    description: 'Perguntas em linguagem natural sobre o banco.',
    icon: AiChat02Icon,
    Component: AssistantSection,
  },
  {
    id: 'updates',
    label: 'Atualizações',
    description: 'Como o Octapus procura versões novas.',
    icon: Download04Icon,
    Component: UpdatesSection,
  },
  {
    id: 'diagnostics',
    label: 'Diagnóstico',
    description: 'O registro do que o app fez, para investigar um problema.',
    icon: Bug01Icon,
    Component: DiagnosticsSection,
  },
];
