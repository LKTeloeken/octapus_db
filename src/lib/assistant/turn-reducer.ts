import type {
  AssistantMention,
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
  /** O modelo pediu mais estrutura: o app busca no catálogo e pergunta de novo */
  | 'structure'
  /** Guard e preflight da consulta gerada */
  | 'checking'
  /** Execução automática (só com `assistantAutoExecute`) */
  | 'executing'
  /** Rate limit: esperando o `Retry-After` */
  | 'waiting'
  /** O modelo perguntou (`ask_user`): esperando a resposta do usuário */
  | 'asking'
  | 'finished';

export interface AttemptFailure {
  stage: AssistantStage;
  failureClass: FailureClass | null;
  /** Mensagem do banco ou do guard, como o app viu (fica na máquina) */
  message: string;
}

/** O que o app achou para um pedido de estrutura do modelo */
export interface StructureLookup {
  requested: number;
  found: number;
  /** Os não encontrados, como o modelo escreveu */
  missing: string[];
  truncated: boolean;
  /** O teto de pedidos do turno já tinha sido atingido: nada foi buscado */
  limitReached: boolean;
}

/** A pergunta do modelo ao usuário (`kind: ask_user`) e a resposta */
export interface LiveQuestion {
  question: string;
  /** Sugestões de resposta; vazio = resposta livre */
  choices: string[];
  /** Chega pelo `userReplied`; `null` enquanto espera */
  reply: string | null;
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
  /** A tentativa foi um pedido de estrutura (evento `structureLookup`) */
  lookup: StructureLookup | null;
  /** A tentativa foi uma pergunta ao usuário (evento `question`) */
  ask: LiveQuestion | null;
}

/**
 * Tentativa que é (ou está virando) um pedido de estrutura: chamada
 * utilitária, não é resposta nem falha
 */
export const isStructureAttempt = (attempt: LiveAttempt) =>
  attempt.lookup !== null || attempt.partial.kind === 'structure_request';

/**
 * Tentativa que é (ou está virando) uma pergunta ao usuário: também não é
 * resposta nem falha. A que passou do teto do turno não chega à tela (sem
 * `question`), mas também não conta
 */
export const isQuestionAttempt = (attempt: LiveAttempt) =>
  attempt.ask !== null || attempt.partial.kind === 'ask_user';

/** O que o evento `context` disse da estrutura enviada ao modelo */
export interface TurnContext {
  tables: number;
  /** Recorte da estrutura (RN-26) */
  truncated: boolean;
  /** Citadas com `@` que não existem no banco */
  missingMentions: AssistantMention[];
  /** Citadas que entraram encurtadas (sem algumas colunas ou nomes) */
  shortenedMentions: AssistantMention[];
}

export interface LiveTurn {
  clientTurnId: string;
  question: string;
  autoExecute: boolean;
  turnId: string | null;
  phase: TurnPhase;
  context: TurnContext | null;
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
        context: {
          tables: event.tables,
          truncated: event.truncated,
          missingMentions: event.missingMentions,
          shortenedMentions: event.shortenedMentions,
        },
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
            lookup: null,
            ask: null,
          },
        ],
      };

    case 'delta': {
      const last = turn.attempts.at(-1);
      if (!last || last.output) return turn;
      const raw = last.raw + event.text;
      const partial = extractPartialOutput(raw);
      return {
        ...turn,
        phase:
          partial.kind === 'structure_request' ? 'structure' : 'generating',
        attempts: [...turn.attempts.slice(0, -1), { ...last, raw, partial }],
      };
    }

    case 'structureLookup': {
      const lookup: StructureLookup = {
        requested: event.requested,
        found: event.found,
        missing: event.missing,
        truncated: event.truncated,
        limitReached: event.limitReached,
      };
      return {
        ...turn,
        // Até a próxima tentativa começar
        phase: 'structure',
        // A estrutura acrescentada não coube inteira: também é recorte (RN-26)
        context:
          turn.context && event.truncated
            ? { ...turn.context, truncated: true }
            : turn.context,
        attempts: patchAttempt(turn, event.attemptId, () => ({ lookup })),
      };
    }

    case 'question':
      return {
        ...turn,
        // Até a resposta chegar
        phase: 'asking',
        attempts: patchAttempt(turn, event.attemptId, () => ({
          ask: {
            question: event.question,
            choices: event.choices,
            reply: null,
          },
        })),
      };

    case 'userReplied':
      return {
        ...turn,
        phase: 'generating',
        attempts: patchAttempt(turn, event.attemptId, attempt => ({
          ask: attempt.ask ? { ...attempt.ask, reply: event.reply } : null,
        })),
      };

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
