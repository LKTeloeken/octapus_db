import { describe, expect, it } from 'vitest';
import type {
  AssistantAttempt,
  AssistantTurn,
  AssistantTurnEvent,
  ModelOutput,
} from '@/api/types/assistant.types';
import {
  failTurn,
  finishTurn,
  reduceTurn,
  startTurn,
  type LiveTurn,
} from '@/lib/assistant/turn-reducer';
import { attemptFailure, liveTurnView, serverTurnView } from './turn-view';

const QUERY: ModelOutput = {
  kind: 'query',
  language: 'sql',
  query: 'SELECT count(*) FROM "users"',
  explanation: 'Conta os usuários.',
  assumptions: ['Todos contam'],
  clarificationQuestion: null,
  structureRequest: null,
};

const run = (turn: LiveTurn, events: AssistantTurnEvent[], now = 1_000) =>
  events.reduce((state, event) => reduceTurn(state, event, now), turn);

const streamOf = (output: ModelOutput): AssistantTurnEvent[] =>
  (JSON.stringify(output).match(/[\s\S]{1,7}/g) ?? []).map(text => ({
    type: 'delta',
    text,
  }));

describe('reduceTurn', () => {
  it('mostra a resposta parcial e troca pela final no output', () => {
    let turn = run(startTurn('c1', 'quantos?', true), [
      { type: 'turnOpened', turnId: 't1' },
      {
        type: 'context',
        tables: 4,
        truncated: true,
        missingMentions: [{ schema: 'public', table: 'ghost' }],
        shortenedMentions: [{ schema: 'public', table: null }],
      },
      { type: 'attemptStarted', attemptId: 'a1', index: 0 },
      ...streamOf(QUERY).slice(0, 6),
    ]);
    let view = liveTurnView(turn);
    expect(view.streaming).toBe(true);
    expect(view.phase).toBe('generating');
    expect(QUERY.query?.startsWith(view.output?.query ?? 'x')).toBe(true);
    expect(view.truncated).toBe(true);
    expect(view.missingMentions).toEqual([
      { schema: 'public', table: 'ghost' },
    ]);
    expect(view.shortenedMentions).toEqual([{ schema: 'public', table: null }]);

    turn = run(turn, [{ type: 'output', attemptId: 'a1', output: QUERY }]);
    view = liveTurnView(turn);
    expect(view.streaming).toBe(false);
    expect(view.output?.query).toBe(QUERY.query);
    expect(view.phase).toBe('checking');
  });

  it('só marca executada com o result (RN-21)', () => {
    const base = run(startTurn('c1', 'quantos?', true), [
      { type: 'attemptStarted', attemptId: 'a1', index: 0 },
      { type: 'output', attemptId: 'a1', output: QUERY },
      {
        type: 'plan',
        attemptId: 'a1',
        plan: { estimatedRows: 1, estimatedCost: 1, plan: '[]' },
      },
    ]);
    expect(base.phase).toBe('executing');
    expect(liveTurnView(base).executedInTurn).toBe(false);

    const finish = {
      turnId: 't1',
      status: 'answered' as const,
      attemptId: 'a1',
      output: QUERY,
      errorCode: null,
    };
    // Revisão: terminou sem result → não executada
    expect(liveTurnView(finishTurn(base, finish)).executedInTurn).toBe(false);

    const executed = run(base, [
      {
        type: 'result',
        attemptId: 'a1',
        result: {
          columns: [],
          rows: [],
          rowCount: 0,
          totalCount: null,
          hasMore: false,
          executionTimeMs: 1,
          editableInfo: null,
        },
      },
    ]);
    expect(liveTurnView(finishTurn(executed, finish)).executedInTurn).toBe(
      true,
    );
  });

  it('sem execução automática o plano não vira "executando"', () => {
    const turn = run(startTurn('c1', 'q', false), [
      { type: 'attemptStarted', attemptId: 'a1', index: 0 },
      { type: 'output', attemptId: 'a1', output: QUERY },
      {
        type: 'plan',
        attemptId: 'a1',
        plan: { estimatedRows: 1, estimatedCost: 1, plan: '[]' },
      },
    ]);
    expect(turn.phase).toBe('checking');
  });

  it('a correção deixa a tentativa anterior como falha', () => {
    const fixed = { ...QUERY, query: 'SELECT 1' };
    const turn = finishTurn(
      run(startTurn('c1', 'q', true), [
        { type: 'attemptStarted', attemptId: 'a1', index: 0 },
        { type: 'output', attemptId: 'a1', output: QUERY },
        {
          type: 'stageFailed',
          attemptId: 'a1',
          stage: 'preflight',
          failureClass: 'unknown_object',
          message: 'column "x" does not exist',
        },
        { type: 'attemptStarted', attemptId: 'a2', index: 1 },
        { type: 'output', attemptId: 'a2', output: fixed },
      ]),
      {
        turnId: 't1',
        status: 'answered',
        attemptId: 'a2',
        output: fixed,
        errorCode: null,
      },
    );
    const view = liveTurnView(turn);
    expect(view.output?.query).toBe('SELECT 1');
    expect(view.failure).toBeNull();
    expect(view.failedAttempts).toEqual([
      {
        attemptId: 'a1',
        index: 0,
        query: QUERY.query,
        failure: {
          stage: 'preflight',
          failureClass: 'unknown_object',
          message: 'column "x" does not exist',
        },
      },
    ]);
  });

  it('no limite esgotado mostra a última consulta e por que falhou (RN-24)', () => {
    const turn = finishTurn(
      run(startTurn('c1', 'q', true), [
        { type: 'attemptStarted', attemptId: 'a1', index: 0 },
        { type: 'output', attemptId: 'a1', output: QUERY },
        {
          type: 'stageFailed',
          attemptId: 'a1',
          stage: 'execution',
          failureClass: 'timeout',
          message: 'canceling statement due to statement timeout',
        },
      ]),
      {
        turnId: 't1',
        status: 'exhausted',
        attemptId: 'a1',
        output: QUERY,
        errorCode: null,
      },
    );
    const view = liveTurnView(turn);
    expect(view.outcome).toBe('exhausted');
    expect(view.output?.query).toBe(QUERY.query);
    expect(view.failure?.stage).toBe('execution');
    expect(view.executedInTurn).toBe(false);
  });

  it('espera do rate limit e erro do comando', () => {
    const waiting = run(startTurn('c1', 'q', true), [
      { type: 'waiting', seconds: 3 },
    ]);
    expect(waiting.phase).toBe('waiting');
    expect(waiting.waitUntil).toBe(4_000);

    const failed = failTurn(waiting, {
      code: 'ASSISTANT_QUOTA_EXCEEDED',
      message: 'quota',
    });
    const view = liveTurnView(failed);
    expect(view.outcome).toBe('failed');
    expect(view.errorCode).toBe('ASSISTANT_QUOTA_EXCEEDED');
    expect(view.waitUntil).toBeNull();
    // Depois do fim, nada muda
    expect(reduceTurn(failed, { type: 'waiting', seconds: 1 }, 0)).toBe(failed);
  });

  it('tentativa sem output (saída inválida) entra como falha da resposta', () => {
    const turn = finishTurn(
      run(startTurn('c1', 'q', true), [
        { type: 'attemptStarted', attemptId: 'a1', index: 0 },
        { type: 'delta', text: '{"kind":"qu' },
        { type: 'attemptStarted', attemptId: 'a2', index: 1 },
        { type: 'output', attemptId: 'a2', output: QUERY },
      ]),
      {
        turnId: 't1',
        status: 'answered',
        attemptId: 'a2',
        output: QUERY,
        errorCode: null,
      },
    );
    expect(liveTurnView(turn).failedAttempts[0].failure?.stage).toBe('output');
  });
});

const attempt = (patch: Partial<AssistantAttempt>): AssistantAttempt => ({
  id: 'a1',
  index: 0,
  status: 'generated',
  output: QUERY,
  usage: null,
  latencyMs: null,
  outcome: null,
  createdAt: '2026-10-20T00:00:00Z',
  ...patch,
});

const stage = (
  ran: boolean,
  ok: boolean | null,
  error: string | null = null,
) => ({
  ran,
  ok,
  failureClass: ok === false ? ('syntax' as const) : null,
  errorSanitized: error,
  durationMs: null,
});

describe('serverTurnView', () => {
  const turn = (patch: Partial<AssistantTurn>): AssistantTurn => ({
    id: 't1',
    index: 0,
    question: 'quantos?',
    language: 'pt',
    status: 'answered',
    createdAt: '2026-10-20T00:00:00Z',
    attempts: [],
    feedback: null,
    ...patch,
  });

  it('executada: só pelo outcome', () => {
    const view = serverTurnView(
      turn({
        attempts: [
          attempt({
            outcome: {
              guard: { allowed: true, reason: null },
              preflight: stage(true, true),
              execution: stage(true, true),
              rowCount: 12,
              autoExecute: true,
              userEdited: false,
            },
          }),
        ],
      }),
      null,
    );
    expect(view.executedInTurn).toBe(true);
    expect(view.recordedRowCount).toBe(12);
    expect(view.output?.query).toBe(QUERY.query);
  });

  it('revisão: não executada', () => {
    const view = serverTurnView(
      turn({
        attempts: [
          attempt({
            outcome: {
              guard: { allowed: true, reason: null },
              preflight: stage(true, true),
              execution: stage(false, null),
              rowCount: null,
              autoExecute: false,
              userEdited: false,
            },
          }),
        ],
      }),
      null,
    );
    expect(view.executedInTurn).toBe(false);
    expect(view.recordedRowCount).toBeNull();
  });

  it('tentativas anteriores com a etapa que falhou', () => {
    const view = serverTurnView(
      turn({
        attempts: [
          attempt({ id: 'a2', index: 1 }),
          attempt({
            id: 'a1',
            index: 0,
            outcome: {
              guard: { allowed: true, reason: null },
              preflight: stage(true, false, '42703: column ? does not exist'),
              execution: stage(false, null),
              rowCount: null,
              autoExecute: true,
              userEdited: false,
            },
          }),
        ],
      }),
      {
        tables: 3,
        truncated: false,
        missingMentions: [{ schema: null, table: 'ghost' }],
        shortenedMentions: [],
      },
    );
    expect(view.attemptId).toBe('a2');
    // O contexto da sessão volta à visão do histórico
    expect(view.truncated).toBe(false);
    expect(view.missingMentions).toEqual([{ schema: null, table: 'ghost' }]);
    expect(view.failedAttempts).toHaveLength(1);
    expect(view.failedAttempts[0].failure).toMatchObject({
      stage: 'preflight',
      message: '42703: column ? does not exist',
    });
  });

  it('kind_code não conta como recusa do guard', () => {
    expect(
      attemptFailure(
        attempt({
          outcome: {
            guard: { allowed: false, reason: 'kind_code' },
            preflight: stage(false, null),
            execution: stage(false, null),
            rowCount: null,
            autoExecute: false,
            userEdited: false,
          },
        }),
      ),
    ).toBeNull();
    expect(
      attemptFailure(attempt({ status: 'invalid_output', output: null })),
    ).toMatchObject({ stage: 'output' });
  });
});

const REQUEST: ModelOutput = {
  kind: 'structure_request',
  language: null,
  query: null,
  explanation: 'Preciso das colunas de pedidos.',
  assumptions: [],
  clarificationQuestion: null,
  structureRequest: ['sales.orders', 'ghost'],
};

const ANSWER: ModelOutput = {
  kind: 'answer',
  language: null,
  query: null,
  explanation:
    'Um índice acelera a busca.\n\n```sql\nCREATE INDEX i ON t (a);\n```',
  assumptions: [],
  clarificationQuestion: null,
  structureRequest: null,
};

describe('pedido de estrutura e resposta em texto', () => {
  it('o pedido de estrutura não é resposta nem falha; o recorte avisa', () => {
    let turn = run(startTurn('c1', 'pedidos por cliente', true), [
      {
        type: 'context',
        tables: 30,
        truncated: false,
        missingMentions: [],
        shortenedMentions: [],
      },
      { type: 'attemptStarted', attemptId: 'a1', index: 0 },
      ...streamOf(REQUEST).slice(0, 8),
    ]);
    // O kind já chegou: "Consultando a estrutura…", sem texto parcial na tela
    expect(turn.phase).toBe('structure');
    expect(liveTurnView(turn).output).toBeNull();

    turn = run(turn, [
      ...streamOf(REQUEST).slice(8),
      {
        type: 'structureLookup',
        attemptId: 'a1',
        requested: 2,
        found: 1,
        missing: ['ghost'],
        truncated: true,
        limitReached: false,
      },
    ]);
    expect(turn.phase).toBe('structure');
    let view = liveTurnView(turn);
    expect(view.output).toBeNull();
    expect(view.truncated).toBe(true);
    expect(view.structureLookups).toEqual([
      { requested: 2, found: 1, missing: ['ghost'], limitReached: false },
    ]);

    turn = finishTurn(
      run(turn, [
        { type: 'attemptStarted', attemptId: 'a2', index: 1 },
        { type: 'output', attemptId: 'a2', output: QUERY },
      ]),
      {
        turnId: 't1',
        status: 'answered',
        attemptId: 'a2',
        output: QUERY,
        errorCode: null,
      },
    );
    view = liveTurnView(turn);
    expect(view.output?.query).toBe(QUERY.query);
    expect(view.failedAttempts).toEqual([]);
    expect(view.structureLookups).toHaveLength(1);
  });

  it('resposta em texto: replied, sem consulta, avaliável', () => {
    const turn = finishTurn(
      run(startTurn('c1', 'o que é um índice?', true), [
        { type: 'turnOpened', turnId: 't1' },
        { type: 'attemptStarted', attemptId: 'a1', index: 0 },
        ...streamOf(ANSWER),
        { type: 'output', attemptId: 'a1', output: ANSWER },
      ]),
      {
        turnId: 't1',
        status: 'replied',
        attemptId: 'a1',
        output: ANSWER,
        errorCode: null,
      },
    );
    const view = liveTurnView(turn);
    expect(view.outcome).toBe('replied');
    expect(view.output?.kind).toBe('answer');
    expect(view.output?.query).toBeNull();
    expect(view.failure).toBeNull();
  });

  it('histórico: o pedido fica de fora da resposta, com os nomes contados', () => {
    const view = serverTurnView(
      {
        id: 't1',
        index: 0,
        question: 'pedidos por cliente',
        language: 'pt',
        status: 'replied',
        createdAt: '2026-10-20T00:00:00Z',
        attempts: [
          attempt({
            id: 'a1',
            index: 0,
            output: REQUEST,
            outcome: {
              guard: { allowed: false, reason: 'kind_structure_request' },
              preflight: stage(false, null),
              execution: stage(false, null),
              rowCount: null,
              autoExecute: true,
              userEdited: false,
            },
          }),
          attempt({ id: 'a2', index: 1, output: ANSWER }),
        ],
        feedback: null,
      },
      null,
    );
    expect(view.outcome).toBe('replied');
    expect(view.attemptId).toBe('a2');
    expect(view.failedAttempts).toEqual([]);
    expect(view.structureLookups).toEqual([
      { requested: 2, found: null, missing: null, limitReached: false },
    ]);
  });
});
