import { useAssistantShortcuts } from './use-assistant-shortcuts';

/** Liga os atalhos do assistente no shell (nada a desenhar) */
export function AssistantShortcutsHost() {
  useAssistantShortcuts();
  return null;
}
