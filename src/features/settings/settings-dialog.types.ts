import type { IconSvgElement } from '@hugeicons/react';
import type { ComponentType } from 'react';

export interface SettingsDialogProps {
  open: boolean;
  onOpenChange: (open: boolean) => void;
}

/** Uma seção do menu lateral das configurações (ver settings-sections.ts) */
export interface SettingsSection {
  id: string;
  label: string;
  /** Linha sob o título da seção */
  description: string;
  icon: IconSvgElement;
  Component: ComponentType;
}
