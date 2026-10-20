import { describe, expect, it } from 'vitest';
import { PERSISTED_DOMAINS } from '@/providers/query-provider';
import { queryKeys } from './keys';

describe('keys do assistente', () => {
  const keys = [
    queryKeys.assistant,
    queryKeys.assistantStatus,
    queryKeys.assistantSession,
    queryKeys.assistantMe,
    queryKeys.assistantConsent,
    queryKeys.assistantModels,
    queryKeys.assistantConversations,
    queryKeys.assistantConversation('c1'),
  ];

  it('nenhuma vai para o cache em disco (RN-01)', () => {
    for (const key of keys) {
      expect(PERSISTED_DOMAINS.has(String(key[0]))).toBe(false);
    }
  });

  it('todas ficam sob o prefixo do assistente', () => {
    for (const key of keys) {
      expect(key[0]).toBe(queryKeys.assistant[0]);
    }
    for (const key of [
      queryKeys.assistantMe,
      queryKeys.assistantConsent,
      queryKeys.assistantModels,
      queryKeys.assistantConversations,
      queryKeys.assistantConversation('c1'),
    ]) {
      expect(key.slice(0, 2)).toEqual([...queryKeys.assistantSession]);
    }
  });
});
