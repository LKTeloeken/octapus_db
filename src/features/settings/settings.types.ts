import type { IconSvgElement } from '@hugeicons/react';
import type { ComponentType } from 'react';

/**
 * Uma seção do diálogo de configurações.
 *
 * Para adicionar uma nova: crie o componente em `sections/`, e registre uma
 * entrada em `settings-sections.ts`. Nada mais precisa mudar — o diálogo monta
 * a navegação a partir do registro.
 */
export interface SettingsSection {
  id: string;
  label: string;
  icon: IconSvgElement;
  Component: ComponentType;
}

export interface SettingsDialogProps {
  open: boolean;
  onClose: () => void;
}
