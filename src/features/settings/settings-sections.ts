import { ShieldKeyIcon } from '@hugeicons/core-free-icons';
import { SecuritySection } from './sections/security-section';
import type { SettingsSection } from './settings.types';

/** Registro das seções. Adicionar uma configuração nova = adicionar uma linha. */
export const SETTINGS_SECTIONS: SettingsSection[] = [
  {
    id: 'security',
    label: 'Segurança',
    icon: ShieldKeyIcon,
    Component: SecuritySection,
  },
];
