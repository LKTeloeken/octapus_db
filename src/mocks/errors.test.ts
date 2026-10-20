import { describe, expect, it } from 'vitest';
import { toBackendError } from './errors';

describe('toBackendError', () => {
  it('tira o código do prefixo da mensagem', () => {
    expect(toBackendError('Query error: relation "x" does not exist')).toEqual({
      code: 'QUERY_ERROR',
      message: 'Query error: relation "x" does not exist',
    });
    expect(toBackendError('Not found: Server with id 9 not found').code).toBe(
      'NOT_FOUND',
    );
    expect(toBackendError('Operation cancelled').code).toBe('CANCELLED');
    expect(toBackendError('Connection timed out').code).toBe(
      'CONNECTION_TIMEOUT',
    );
  });

  it('mensagem sem prefixo é External, como no Rust', () => {
    expect(toBackendError('O pg_dump não foi encontrado.')).toEqual({
      code: 'EXTERNAL_ERROR',
      message: 'O pg_dump não foi encontrado.',
    });
  });

  it('erros do assistente levam o código da API', () => {
    expect(toBackendError('Assistant not configured').code).toBe(
      'ASSISTANT_NOT_CONFIGURED',
    );
    expect(toBackendError('Assistant unauthorized: not signed in').code).toBe(
      'ASSISTANT_UNAUTHORIZED',
    );
    expect(
      toBackendError('Assistant model_selection_forbidden: not free').code,
    ).toBe('ASSISTANT_MODEL_SELECTION_FORBIDDEN');
    expect(
      toBackendError('Assistant unavailable: connection refused').code,
    ).toBe('ASSISTANT_UNAVAILABLE');
  });

  it('aceita Error lançado por engano num handler', () => {
    expect(toBackendError(new Error('Invalid state: x')).code).toBe(
      'INVALID_STATE',
    );
  });
});
