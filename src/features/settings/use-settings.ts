import { useEffect, useState } from 'react';
import { SETTINGS_SECTIONS } from './settings-sections';
import type { SettingsDialogProps } from './settings.types';

export const useSettings = ({ open }: Pick<SettingsDialogProps, 'open'>) => {
  const [activeId, setActiveId] = useState(SETTINGS_SECTIONS[0].id);

  // Reabrir sempre começa na primeira seção.
  useEffect(() => {
    if (open) setActiveId(SETTINGS_SECTIONS[0].id);
  }, [open]);

  const active =
    SETTINGS_SECTIONS.find(section => section.id === activeId) ??
    SETTINGS_SECTIONS[0];

  return { sections: SETTINGS_SECTIONS, active, setActiveId };
};
