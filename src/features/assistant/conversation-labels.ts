import type { AssistantConversationSummary } from '@/api/types/assistant.types';

const PARADIGM_LABELS: Record<string, string> = {
  postgres: 'PostgreSQL',
  sqlite: 'SQLite',
  mongodb: 'MongoDB',
  redis: 'Redis',
};

const dateFormat = new Intl.DateTimeFormat('pt-BR', {
  dateStyle: 'short',
  timeStyle: 'short',
});

/** A linha de baixo de uma conversa do histórico: banco, modelo e quando */
export const conversationDetail = (item: AssistantConversationSummary) =>
  `${PARADIGM_LABELS[item.paradigm] ?? item.paradigm} · ${item.model.alias} · ${dateFormat.format(new Date(item.updatedAt))}`;
