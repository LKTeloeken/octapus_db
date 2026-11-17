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
  it('leva a severidade e a mensagem, sem hora', () => {
    expect(formatMessageText(message())).toBe('NOTICE: olá');
  });

  it('acrescenta as linhas extras na ordem da tela', () => {
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
    expect(text).toBe(
      [
        'ERROR: falhou',
        'DETAIL: d',
        'HINT: h',
        'CONTEXT: PL/pgSQL function f() line 3',
        'SQLSTATE: P0001',
        'POSITION: 0',
      ].join('\n'),
    );
  });

  it('junta as mensagens da execução e separa as execuções', () => {
    const entry = (id: string, texts: string[]) => ({
      id,
      startedAt: 0,
      query: 'select 1',
      messages: texts.map(text => message({ message: text })),
    });

    expect(formatEntryText(entry('a', ['um', 'dois']))).toBe(
      'NOTICE: um\nNOTICE: dois',
    );
    expect(
      formatLogText([entry('a', ['um']), entry('b', []), entry('c', ['três'])]),
    ).toBe('NOTICE: um\n\nNOTICE: três');
  });
});
