import { QueryClient, dehydrate } from '@tanstack/react-query';
import { describe, expect, it, vi } from 'vitest';
import type { AssistantConversation } from '@/api/types/assistant.types';
import { queryKeys } from '@/queries/keys';
import {
  type FeedbackChange,
  feedbackMutationOptions,
} from '@/queries/use-assistant-feedback';
import { PERSIST_DEHYDRATE_OPTIONS } from './query-provider';

vi.mock('@/api/assistant', () => ({
  setAssistantFeedback: vi.fn(),
  clearAssistantFeedback: vi.fn(),
}));

/** Pergunta, resposta, linha do resultado e comentário: nada disso vai ao disco */
const CANARY = 'cnry-b10-assistente';
/** Prova de que o cache foi gravado de fato (não é um teste vazio) */
const VISIBLE = 'srv-visivel-no-cache';

/** Uma mutação pausada (sem rede, ou na fila do `scope`): a que o persister grava */
const paused = <T>(variables: T) => ({
  context: undefined,
  data: undefined,
  error: null,
  failureCount: 0,
  failureReason: null,
  isPaused: true,
  status: 'pending' as const,
  variables,
  submittedAt: Date.now(),
});

describe('cache persistido do React Query (RN-01, critério 8)', () => {
  it('o que vai ao IndexedDB não leva nada do assistente', () => {
    const client = new QueryClient();

    client.setQueryData(queryKeys.servers, [{ id: 1, name: VISIBLE }]);
    client.setQueryData(queryKeys.assistantConversation('c-1'), {
      id: 'c-1',
      turns: [
        {
          id: 't-1',
          question: `quanto pagou ${CANARY}?`,
          attempts: [{ output: { query: `SELECT '${CANARY}'` } }],
          feedback: { rating: 'down', comment: CANARY },
        },
      ],
    } as unknown as AssistantConversation);
    client.setQueryData(queryKeys.assistantConversations, {
      pages: [{ items: [{ id: 'c-1', title: CANARY }] }],
    });
    client.setQueryData(queryKeys.assistantMe, { participant: CANARY });

    // A avaliação pausada com o comentário, e uma mutação comum pausada
    client.getMutationCache().build(
      client,
      feedbackMutationOptions(client, 'c-1', 't-1'),
      paused<FeedbackChange>({
        rating: 'down',
        reasons: ['wrong_result'],
        comment: CANARY,
        attemptId: 'a-1',
      }),
    );
    client
      .getMutationCache()
      .build(client, { mutationKey: ['servers'] }, paused({ name: VISIBLE }));

    const persisted = JSON.stringify(
      dehydrate(client, PERSIST_DEHYDRATE_OPTIONS),
    );

    expect(persisted).toContain(VISIBLE);
    expect(persisted).not.toContain(CANARY);
    expect(persisted).not.toContain('"assistant"');
  });
});
