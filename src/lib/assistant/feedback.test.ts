import { describe, expect, it } from 'vitest';
import type {
  AssistantFeedback,
  AssistantFeedbackInput,
} from '@/api/types/assistant.types';
import {
  FEEDBACK_COMMENT_MAX,
  commentLength,
  commentTooLong,
  nextRating,
  normalizeFeedback,
  optimisticFeedback,
  toggleReason,
  withComment,
} from './feedback';

const saved = (patch: Partial<AssistantFeedback> = {}): AssistantFeedback => ({
  rating: 'down',
  reasons: ['wrong_query'],
  comment: 'faltou o filtro',
  attemptId: 'a-1',
  createdAt: '2026-10-20T10:00:00Z',
  updatedAt: '2026-10-20T10:00:00Z',
  ...patch,
});

describe('alternar 👍/👎', () => {
  it('sem avaliação, o clique avalia', () => {
    expect(nextRating(null, 'up', 'a-1')).toEqual({
      rating: 'up',
      reasons: [],
      comment: null,
      attemptId: 'a-1',
    });
  });

  it('o mesmo botão de novo remove', () => {
    expect(nextRating(saved({ rating: 'up' }), 'up', 'a-1')).toBeNull();
    expect(nextRating(saved(), 'down', 'a-1')).toBeNull();
  });

  it('trocar leva o comentário e larga os motivos', () => {
    expect(nextRating(saved(), 'up', 'a-1')).toEqual({
      rating: 'up',
      reasons: [],
      comment: 'faltou o filtro',
      attemptId: 'a-1',
    });
    expect(
      nextRating(saved({ rating: 'up', reasons: [] }), 'down', 'a-1'),
    ).toMatchObject({ rating: 'down', reasons: [] });
  });

  it('motivos ligam e desligam só no 👎', () => {
    expect(toggleReason(saved(), 'slow', 'a-1')?.reasons).toEqual([
      'wrong_query',
      'slow',
    ]);
    expect(toggleReason(saved(), 'wrong_query', 'a-1')?.reasons).toEqual([]);
    expect(toggleReason(saved({ rating: 'up' }), 'slow', 'a-1')).toBeNull();
    expect(toggleReason(null, 'slow', 'a-1')).toBeNull();
  });

  it('salvar o comentário mantém a nota e os motivos', () => {
    expect(withComment(saved(), 'novo', 'a-1')).toEqual({
      rating: 'down',
      reasons: ['wrong_query'],
      comment: 'novo',
      attemptId: 'a-1',
    });
  });
});

describe('o que vai para a API', () => {
  it('motivos só no 👎, sem repetir e sem os desconhecidos', () => {
    expect(
      normalizeFeedback({
        rating: 'up',
        reasons: ['slow'],
        comment: null,
        attemptId: null,
      }).reasons,
    ).toEqual([]);
    expect(
      normalizeFeedback({
        rating: 'down',
        reasons: ['slow', 'unknown', 'slow', 'other'],
        comment: null,
        attemptId: null,
      }).reasons,
    ).toEqual(['slow', 'other']);
  });

  it('comentário aparado; só espaços vira null', () => {
    const base = {
      rating: 'up',
      reasons: [],
      attemptId: null,
    } satisfies Partial<AssistantFeedbackInput>;
    expect(normalizeFeedback({ ...base, comment: '  ok \n' }).comment).toBe(
      'ok',
    );
    expect(normalizeFeedback({ ...base, comment: '   ' }).comment).toBeNull();
  });

  it('o otimista guarda o createdAt da primeira avaliação', () => {
    const next = optimisticFeedback(
      { rating: 'up', reasons: [], comment: null, attemptId: 'a-1' },
      saved(),
      '2026-10-20T11:00:00Z',
    );
    expect(next.createdAt).toBe('2026-10-20T10:00:00Z');
    expect(next.updatedAt).toBe('2026-10-20T11:00:00Z');
    expect(
      optimisticFeedback(
        { rating: 'up', reasons: [], comment: null, attemptId: null },
        null,
        'agora',
      ).createdAt,
    ).toBe('agora');
  });
});

describe('limite do comentário', () => {
  it('conta caracteres como a API (emoji = 1), sem as bordas', () => {
    expect(commentLength('  abc  ')).toBe(3);
    expect(commentLength('👍👎')).toBe(2);
  });

  it('2000 passa, 2001 não', () => {
    expect(commentTooLong('a'.repeat(FEEDBACK_COMMENT_MAX))).toBe(false);
    expect(commentTooLong('é'.repeat(FEEDBACK_COMMENT_MAX + 1))).toBe(true);
    expect(commentTooLong('🙂'.repeat(FEEDBACK_COMMENT_MAX))).toBe(false);
    expect(commentTooLong(`  ${'a'.repeat(FEEDBACK_COMMENT_MAX)}  `)).toBe(
      false,
    );
  });
});
