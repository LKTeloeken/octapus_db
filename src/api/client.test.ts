import { beforeEach, describe, expect, it, vi } from 'vitest';
import { invoke } from '@tauri-apps/api/core';
import { ApiError, call, hasErrorCode } from './client';
import { RustCommand } from './commands';

vi.mock('@tauri-apps/api/core', () => ({ invoke: vi.fn() }));

const rejectWith = (value: unknown) =>
  vi.mocked(invoke).mockRejectedValueOnce(value);

async function caught(promise: Promise<unknown>): Promise<ApiError> {
  try {
    await promise;
  } catch (error) {
    if (error instanceof ApiError) return error;
  }
  throw new Error('esperava um ApiError');
}

describe('call', () => {
  beforeEach(() => vi.mocked(invoke).mockReset());

  it('devolve o resultado do comando', async () => {
    vi.mocked(invoke).mockResolvedValueOnce([1, 2]);
    await expect(call(RustCommand.GetAllServers)).resolves.toEqual([1, 2]);
  });

  it('carrega o código e a mensagem do Error do backend', async () => {
    rejectWith({ code: 'QUERY_ERROR', message: 'Query error: boom' });
    const error = await caught(call(RustCommand.ExecuteQuery, { query: 'x' }));

    expect(error.code).toBe('QUERY_ERROR');
    expect(error.message).toBe('Query error: boom');
    expect(error.command).toBe(RustCommand.ExecuteQuery);
    expect(hasErrorCode(error, 'QUERY_ERROR')).toBe(true);
    expect(hasErrorCode(error, 'CANCELLED')).toBe(false);
  });

  it('rejeição em string (Tauri, plugins) vira UNKNOWN', async () => {
    rejectWith('invalid args `serverId` for command `connect`');
    const error = await caught(call(RustCommand.Connect));

    expect(error.code).toBe('UNKNOWN');
    expect(error.message).toBe('invalid args `serverId` for command `connect`');
  });

  it('não confunde um objeto qualquer com o Error do backend', async () => {
    rejectWith({ code: 42 });
    const error = await caught(call(RustCommand.Connect));

    expect(error.code).toBe('UNKNOWN');
    expect(error.message).toBe('Unknown backend error');
  });

  it('hasErrorCode só aceita ApiError', () => {
    expect(hasErrorCode(new Error('x'), 'UNKNOWN')).toBe(false);
    expect(hasErrorCode('Query error: x', 'QUERY_ERROR')).toBe(false);
  });
});
