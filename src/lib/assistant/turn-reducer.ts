import type {
  AssistantStage,
  AssistantTurnEvent,
  AssistantTurnFinish,
  FailureClass,
  ModelOutput,
} from '@/api/types/assistant.types';
import {
  EMPTY_PARTIAL_OUTPUT,
  extractPartialOutput,
  type PartialModelOutput,
} from './partial-output';

/**
 * O turno ao vivo, montado a partir dos eventos do canal do
 * `assistant_turn_ask`. Função pura: a tela guarda o estado (em memória, no
 * `assistant-store`) e chama `reduceTurn` a cada evento.
 *
 * Regra que este estado garante (RN-21): uma tentativa só fica `executed`
 * quando chega o `result`. Plano, preflight ou "executando…" nunca contam.
 */

/** Em que parte do turno estamos — vira o rótulo ao lado do polvo */
export type TurnPhase =
  /** Abrindo o turno e montando o contexto da estrutura */
  | 'opening'
  /** Esperando o modelo (antes e durante o stream) */
  | 'generating'
  /** Guard e preflight da consulta gerada */
  | 'checking'
  /** Execução automática (só com `assistantAutoExecute`) */
  | 'executing'
  /** Rate limit: esperando o `Retry-After` */
  | 'waiting'
  | 'finished';

export interface AttemptFailure {
  stage: AssistantStage;
  failureClass: FailureClass | null;
  /** Mensagem do banco ou do guard, como o app viu (fica na máquina) */
  message: string;
}

export interface LiveAttempt {
  attemptId: string;
  index: number;
  /** Texto cru do stream (JSON parcial do `ModelOutput`) */
  raw: string;
  partial: PartialModelOutput;
  /** A resposta final conferida (evento `output`) */
  output: ModelOutput | null;
  failure: AttemptFailure | null;
  planned: boolean;
  /** Só com o evento `result` */
  executed: boolean;
}

export interface LiveTurn {
  clientTurnId: string;
  question: string;
  autoExecute: boolean;
  turnId: string | null;
  phase: TurnPhase;
  context: { tables: number; truncated: boolean } | null;
  attempts: LiveAttempt[];
  /** Fim da espera do rate limit (epoch ms) */
  waitUntil: number | null;
  finish: AssistantTurnFinish | null;
  /** O comando rejeitou (cota, API fora, conversa sumiu…) */
  error: { code: string; message: string } | null;
}

export function startTurn(
  clientTurnId: string,
  question: string,
  autoExecute: boolean,
): LiveTurn {
  return {
    clientTurnId,
    question,
    autoExecute,
    turnId: null,
    phase: 'opening',
    context: null,
    attempts: [],
    waitUntil: null,
    finish: null,
    error: null,
  };
}

function patchAttempt(
  turn: LiveTurn,
  attemptId: string,
  patch: (attempt: LiveAttempt) => Partial<LiveAttempt>,
): LiveAttempt[] {
  return turn.attempts.map(attempt =>
    attempt.attemptId === attemptId
      ? { ...attempt, ...patch(attempt) }
      : attempt,
  );
}

export function reduceTurn(
  turn: LiveTurn,
  event: AssistantTurnEvent,
  now: number,
): LiveTurn {
  // Evento atrasado depois do fim não muda nada
  if (turn.phase === 'finished') return turn;

  switch (event.type) {
    case 'turnOpened':
      return { ...turn, turnId: event.turnId };

    case 'context':
      return {
        ...turn,
        context: { tables: event.tables, truncated: event.truncated },
        phase: 'generating',
      };

    case 'attemptStarted':
      return {
        ...turn,
        phase: 'generating',
        waitUntil: null,
        attempts: [
          ...turn.attempts,
          {
            attemptId: event.attemptId,
            index: event.index,
            raw: '',
            partial: EMPTY_PARTIAL_OUTPUT,
            output: null,
            failure: null,
            planned: false,
            executed: false,
          },
        ],
      };

    case 'delta': {
      const last = turn.attempts.at(-1);
      if (!last || last.output) return turn;
      const raw = last.raw + event.text;
      return {
        ...turn,
        phase: 'generating',
        attempts: [
          ...turn.attempts.slice(0, -1),
          { ...last, raw, partial: extractPartialOutput(raw) },
        ],
      };
    }

    case 'output':
      return {
        ...turn,
        phase: event.output.kind === 'query' ? 'checking' : 'generating',
        attempts: patchAttempt(turn, event.attemptId, () => ({
          output: event.output,
        })),
      };

    case 'stageFailed':
      return {
        ...turn,
        phase: 'generating',
        attempts: patchAttempt(turn, event.attemptId, () => ({
          failure: {
            stage: event.stage,
            failureClass: event.failureClass,
            message: event.message,
          },
        })),
      };

    case 'plan':
      return {
        ...turn,
        phase: turn.autoExecute ? 'executing' : 'checking',
        attempts: patchAttempt(turn, event.attemptId, () => ({
          planned: true,
        })),
      };

    case 'result':
      return {
        ...turn,
        attempts: patchAttempt(turn, event.attemptId, () => ({
          executed: true,
        })),
      };

    case 'waiting':
      return {
        ...turn,
        phase: 'waiting',
        waitUntil: now + event.seconds * 1000,
      };
  }
}

export function finishTurn(
  turn: LiveTurn,
  finish: AssistantTurnFinish,
): LiveTurn {
  return {
    ...turn,
    turnId: turn.turnId ?? finish.turnId,
    phase: 'finished',
    waitUntil: null,
    finish,
  };
}

export function failTurn(
  turn: LiveTurn,
  error: { code: string; message: string },
): LiveTurn {
  return { ...turn, phase: 'finished', waitUntil: null, error };
}
