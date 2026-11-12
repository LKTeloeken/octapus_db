import { useMemo, useState } from 'react';
import { useAssistantAvailable } from '@/queries/use-assistant-status';
import { SETTINGS_SECTIONS } from './settings-sections';

export const useSettingsDialog = () => {
  const assistantAvailable = useAssistantAvailable();
  // Sem o assistente (fora do canal beta ou build sem a API), a seção some
  const sections = useMemo(
    () =>
      SETTINGS_SECTIONS.filter(
        section => section.id !== 'assistant' || assistantAvailable,
      ),
    [assistantAvailable],
  );

  const [activeId, setActiveId] = useState(sections[0].id);
  const active =
    sections.find(section => section.id === activeId) ?? sections[0];

  return { sections, active, setActiveId };
};
