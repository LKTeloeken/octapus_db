import { describe, expect, it } from 'vitest';
import type { QueryMessage } from '@/api/types/query.types';
import {
  formatEntryText,
  formatLogText,
  formatMessageText,
} from './query-messages-text';

const message = (overrides: Partial<QueryMessage> = {}): QueryMessage => ({
  kind: 'notice',
  severity: 'NOTICE',
  message: 'olá',
  detail: null,
  hint: null,
  context: null,
  sqlState: null,
  position: null,
  timestampMs: 0,
  ...overrides,
});

describe('texto copiado do log de mensagens', () => {
  it('leva só a mensagem, sem hora nem severidade', () => {
    expect(formatMessageText(message())).toBe('olá');
  });

  it('deixa de fora DETAIL, HINT, CONTEXT, SQLSTATE e POSITION', () => {
    const text = formatMessageText(
      message({
        kind: 'error',
        severity: 'ERROR',
        message: 'falhou',
        detail: 'd',
        hint: 'h',
        context: 'PL/pgSQL function f() line 3',
        sqlState: 'P0001',
        position: 0,
      }),
    );
    expect(text).toBe('falhou');
  });

  it('junta as mensagens da execução e separa as execuções', () => {
    const entry = (id: string, texts: string[]) => ({
      id,
      startedAt: 0,
      query: 'select 1',
      messages: texts.map(text => message({ message: text })),
    });

    expect(formatEntryText(entry('a', ['um', 'dois']))).toBe('um\ndois');
    expect(
      formatLogText([entry('a', ['um']), entry('b', []), entry('c', ['três'])]),
    ).toBe('um\n\ntrês');
  });
});
