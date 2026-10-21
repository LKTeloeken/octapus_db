import { MutationObserver, QueryClient } from '@tanstack/react-query';
import { beforeEach, describe, expect, it, vi } from 'vitest';
import { ApiError } from '@/api/client';
import { RustCommand } from '@/api/commands';
import type {
  AssistantConversation,
  AssistantFeedback,
} from '@/api/types/assistant.types';
import { shouldPersistMutation } from '@/providers/query-provider';
import { queryKeys } from './keys';
import { feedbackMutationOptions } from './use-assistant-feedback';

const api = vi.hoisted(() => ({
  setAssistantFeedback: vi.fn(),
  clearAssistantFeedback: vi.fn(),
}));
vi.mock('@/api/assistant', () => api);

const CONVERSATION = 'c-1';
const TURN = 't-1';
const key = queryKeys.assistantConversation(CONVERSATION);

const conversation = (
  feedback: AssistantFeedback | null,
): AssistantConversation =>
  ({
    id: CONVERSATION,
    turns: [
      { id: TURN, index: 0, feedback },
      { id: 't-2', index: 1, feedback: null },
    ],
  }) as unknown as AssistantConversation;

const UP: AssistantFeedback = {
  rating: 'up',
  reasons: [],
  comment: 'boa',
  attemptId: 'a-1',
  createdAt: '2026-10-20T10:00:00Z',
  updatedAt: '2026-10-20T10:00:00Z',
};

/** Promessa que o teste resolve ou rejeita quando quiser */
function deferred() {
  let resolve!: () => void;
  let reject!: (error: unknown) => void;
  const promise = new Promise<void>((res, rej) => {
    resolve = res;
    reject = rej;
  });
  return { promise, resolve, reject };
}

const observe = (client: QueryClient) =>
  new MutationObserver(
    client,
    feedbackMutationOptions(client, CONVERSATION, TURN),
  );

const feedbackIn = (client: QueryClient) =>
  client.getQueryData<AssistantConversation>(key)?.turns[0].feedback ?? null;

describe('avaliação otimista', () => {
  let client: QueryClient;
  let observer: ReturnType<typeof observe>;

  beforeEach(() => {
    vi.clearAllMocks();
    client = new QueryClient();
    client.setQueryData(key, conversation(UP));
    observer = observe(client);
  });

  it('a tela muda antes de o servidor responder', async () => {
    const server = deferred();
    api.setAssistantFeedback.mockReturnValue(server.promise);

    const done = observer.mutate({
      rating: 'down',
      reasons: ['slow'],
      comment: ' boa ',
      attemptId: 'a-1',
    });
    await vi.waitFor(() =>
      expect(feedbackIn(client)).toMatchObject({
        rating: 'down',
        reasons: ['slow'],
        comment: 'boa',
        createdAt: UP.createdAt,
      }),
    );
    // O outro turno não muda
    expect(
      client.getQueryData<AssistantConversation>(key)?.turns[1].feedback,
    ).toBeNull();

    server.resolve();
    await done;
    expect(api.setAssistantFeedback).toHaveBeenCalledWith(TURN, {
      rating: 'down',
      reasons: ['slow'],
      comment: ' boa ',
      attemptId: 'a-1',
    });
  });

  it('remover some com a avaliação na hora', async () => {
    api.clearAssistantFeedback.mockResolvedValue(undefined);
    await observer.mutate(null);
    expect(feedbackIn(client)).toBeNull();
    expect(api.clearAssistantFeedback).toHaveBeenCalledWith(TURN);
  });

  it('servidor recusou: volta ao estado anterior', async () => {
    const refused = new ApiError(
      'Assistant conflict: no answer',
      RustCommand.AssistantFeedbackSet,
      'ASSISTANT_CONFLICT',
    );
    api.setAssistantFeedback.mockRejectedValue(refused);

    await expect(
      observer.mutate({
        rating: 'down',
        reasons: [],
        comment: null,
        attemptId: 'a-1',
      }),
    ).rejects.toBe(refused);
    expect(feedbackIn(client)).toEqual(UP);

    // Remover recusado traz a avaliação de volta
    api.clearAssistantFeedback.mockRejectedValue(refused);
    await expect(observer.mutate(null)).rejects.toBe(refused);
    expect(feedbackIn(client)).toEqual(UP);
  });

  it('cliques seguidos saem em ordem e a conversa só é relida no fim', async () => {
    const first = deferred();
    const order: string[] = [];
    api.setAssistantFeedback.mockImplementation(
      (_turn: string, change: { rating: string }) => {
        order.push(change.rating);
        return change.rating === 'down' ? first.promise : Promise.resolve();
      },
    );
    const invalidate = vi.spyOn(client, 'invalidateQueries');

    const a = observer.mutate({
      rating: 'down',
      reasons: [],
      comment: null,
      attemptId: null,
    });
    const second = observe(client);
    const b = second.mutate({
      rating: 'up',
      reasons: [],
      comment: null,
      attemptId: null,
    });

    await vi.waitFor(() => {
      expect(feedbackIn(client)?.rating).toBe('up');
      expect(order).toEqual(['down']);
    });
    // O segundo espera o primeiro terminar
    await new Promise(resolve => setTimeout(resolve, 20));
    expect(order).toEqual(['down']);

    first.resolve();
    await Promise.all([a, b]);
    expect(order).toEqual(['down', 'up']);
    expect(invalidate).toHaveBeenCalledTimes(1);
    expect(invalidate).toHaveBeenCalledWith({ queryKey: key });
  });

  it('a mutação do assistente nunca vai para o disco (RN-01)', () => {
    const mutation = client.getMutationCache().build(client, {
      ...feedbackMutationOptions(client, CONVERSATION, TURN),
    });
    // Pausada é o que o persister gravaria
    mutation.state.isPaused = true;
    expect(shouldPersistMutation(mutation as never)).toBe(false);

    const other = client
      .getMutationCache()
      .build(client, { mutationKey: ['servers'] });
    other.state.isPaused = true;
    expect(shouldPersistMutation(other as never)).toBe(true);
  });
});
