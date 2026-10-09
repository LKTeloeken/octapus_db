import { useState } from 'react';
import { SETTINGS_SECTIONS } from './settings-sections';

export const useSettingsDialog = () => {
  const [activeId, setActiveId] = useState(SETTINGS_SECTIONS[0].id);
  const active =
    SETTINGS_SECTIONS.find(section => section.id === activeId) ??
    SETTINGS_SECTIONS[0];

  return { sections: SETTINGS_SECTIONS, active, setActiveId };
};
