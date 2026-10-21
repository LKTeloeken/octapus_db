import type {
  AssistantAttempt,
  AssistantFeedback,
  AssistantMention,
  AssistantStage,
  AssistantTurn,
  FailureClass,
  ModelOutput,
} from '@/api/types/assistant.types';
import type { PartialModelOutput } from '@/lib/assistant/partial-output';
import type {
  LiveAttempt,
  LiveTurn,
  TurnContext,
  TurnPhase,
} from '@/lib/assistant/turn-reducer';

/**
 * O que a tela mostra de um turno, venha ele do canal (ao vivo) ou do
 * histórico do servidor. Funções puras.
 */

export type TurnOutcome =
  | 'running'
  | 'answered'
  | 'code'
  | 'clarification'
  | 'exhausted'
  | 'failed'
  | 'cancelled';

/** Onde uma tentativa parou: as etapas do app ou a própria resposta */
export type FailureStage = AssistantStage | 'output';

export interface AttemptFailureView {
  stage: FailureStage;
  failureClass: FailureClass | null;
  message: string | null;
}

export interface FailedAttemptView {
  attemptId: string;
  index: number;
  query: string | null;
  failure: AttemptFailureView | null;
}

export interface TurnView {
  /** `turnId`, ou o `clientTurnId` enquanto o turno não abriu */
  key: string;
  turnId: string | null;
  question: string;
  live: boolean;
  phase: TurnPhase;
  outcome: TurnOutcome;
  /** A tentativa cuja resposta está na tela */
  attemptId: string | null;
  output: PartialModelOutput | null;
  /** A resposta ainda está chegando (parcial) */
  streaming: boolean;
  /** Por que a tentativa da tela não chegou ao resultado, se não chegou */
  failure: AttemptFailureView | null;
  /** As outras tentativas, que falharam (RN-29) */
  failedAttempts: FailedAttemptView[];
  /** Recorte da estrutura (RN-26); `null` = não se sabe (histórico) */
  truncated: boolean | null;
  /** Citadas com `@` que não existem no banco (aviso) */
  missingMentions: AssistantMention[];
  /** Citadas que entraram encurtadas no recorte */
  shortenedMentions: AssistantMention[];
  /**
   * A consulta rodou dentro do turno: o `result` chegou (ao vivo) ou o outcome
   * registrou a execução com sucesso (histórico). Nunca por inferência (RN-21).
   */
  executedInTurn: boolean;
  /** Linhas registradas no outcome (só metadado; o resultado não é guardado) */
  recordedRowCount: number | null;
  errorCode: string | null;
  errorMessage: string | null;
  waitUntil: number | null;
  feedback: AssistantFeedback | null;
}

const toDisplay = (output: ModelOutput): PartialModelOutput => ({
  kind: output.kind,
  query: output.query,
  explanation: output.explanation,
  assumptions: output.assumptions,
  clarificationQuestion: output.clarificationQuestion,
});

const hasContent = (partial: PartialModelOutput) =>
  partial.query !== null ||
  partial.explanation !== null ||
  partial.clarificationQuestion !== null ||
  partial.assumptions.length > 0;

function liveFailure(attempt: LiveAttempt): AttemptFailureView | null {
  if (attempt.failure) {
    return {
      stage: attempt.failure.stage,
      failureClass: attempt.failure.failureClass,
      message: attempt.failure.message,
    };
  }
  if (!attempt.output) {
    return { stage: 'output', failureClass: 'invalid_output', message: null };
  }
  return null;
}

export function liveTurnView(turn: LiveTurn): TurnView {
  const finished = turn.phase === 'finished';
  const last = turn.attempts.at(-1) ?? null;
  const shownId = turn.finish?.attemptId ?? last?.attemptId ?? null;
  const shown =
    turn.attempts.find(attempt => attempt.attemptId === shownId) ?? null;

  let output: PartialModelOutput | null = null;
  let streaming = false;
  if (turn.finish?.output) {
    output = toDisplay(turn.finish.output);
  } else if (shown?.output) {
    output = toDisplay(shown.output);
  } else if (shown && !finished && hasContent(shown.partial)) {
    output = shown.partial;
    streaming = true;
  }

  let outcome: TurnOutcome = 'running';
  if (turn.error) outcome = 'failed';
  else if (turn.finish) outcome = outcomeOf(turn.finish.status);

  return {
    key: turn.turnId ?? turn.clientTurnId,
    turnId: turn.turnId,
    question: turn.question,
    live: true,
    phase: turn.phase,
    outcome,
    attemptId: shownId,
    output,
    streaming,
    // Ao vivo, a tentativa da tela só "falhou" depois do fim: no meio do
    // turno a correção ainda pode vir
    failure:
      finished && shown && outcome !== 'answered' && outcome !== 'cancelled'
        ? shown.failure
          ? liveFailure(shown)
          : null
        : null,
    failedAttempts: turn.attempts
      .filter(attempt => attempt.attemptId !== shownId)
      .filter(attempt => finished || attempt !== last)
      .map(attempt => ({
        attemptId: attempt.attemptId,
        index: attempt.index,
        query: attempt.output?.query ?? attempt.partial.query,
        failure: liveFailure(attempt),
      })),
    truncated: turn.context?.truncated ?? null,
    missingMentions: turn.context?.missingMentions ?? [],
    shortenedMentions: turn.context?.shortenedMentions ?? [],
    executedInTurn: shown?.executed ?? false,
    recordedRowCount: null,
    errorCode: turn.error?.code ?? turn.finish?.errorCode ?? null,
    errorMessage: turn.error?.message ?? null,
    waitUntil: turn.waitUntil,
    feedback: null,
  };
}

function outcomeOf(status: AssistantTurn['status']): TurnOutcome {
  switch (status) {
    case 'answered':
      return 'answered';
    case 'code_delivered':
      return 'code';
    case 'clarification':
      return 'clarification';
    case 'exhausted':
      return 'exhausted';
    case 'cancelled':
      return 'cancelled';
    case 'failed':
    case 'open':
    case 'unknown':
      return 'failed';
  }
}

/** Por que uma tentativa registrada ficou sem resposta */
const ATTEMPT_STATUS_MESSAGES: Partial<
  Record<AssistantAttempt['status'], string>
> = {
  invalid_output: 'resposta fora do formato',
  provider_error: 'o provedor do modelo falhou',
  refused: 'o modelo recusou responder',
  limit_exceeded: 'limite do turno atingido',
  cancelled: 'cancelada',
};

/** O código de erro do fim do turno, deduzido da última tentativa (histórico) */
const ATTEMPT_STATUS_ERROR: Partial<
  Record<AssistantAttempt['status'], string>
> = {
  provider_error: 'ASSISTANT_PROVIDER_ERROR',
  refused: 'ASSISTANT_MODEL_REFUSED',
  limit_exceeded: 'ASSISTANT_TURN_LIMIT_EXCEEDED',
  invalid_output: 'ASSISTANT_INVALID_OUTPUT',
};

/** Onde uma tentativa registrada no servidor parou, pelo status e o outcome */
export function attemptFailure(
  attempt: AssistantAttempt,
): AttemptFailureView | null {
  if (attempt.status !== 'generated') {
    if (attempt.status === 'generating') return null;
    return {
      stage: 'output',
      failureClass:
        attempt.status === 'invalid_output' ? 'invalid_output' : null,
      message: ATTEMPT_STATUS_MESSAGES[attempt.status] ?? null,
    };
  }

  const outcome = attempt.outcome;
  if (!outcome) return null;

  const { guard, preflight, execution } = outcome;
  // `kind_code`/`kind_clarification`: o guard nem rodou, não é recusa
  if (!guard.allowed && !guard.reason?.startsWith('kind_')) {
    return {
      stage: 'guard',
      failureClass: 'guard_rejected',
      message: guard.reason,
    };
  }
  if (preflight.ran && preflight.ok === false) {
    return {
      stage: 'preflight',
      failureClass: preflight.failureClass,
      message: preflight.errorSanitized,
    };
  }
  if (execution.ran && execution.ok === false) {
    return {
      stage: 'execution',
      failureClass: execution.failureClass,
      message: execution.errorSanitized,
    };
  }
  return null;
}

/** `context`: o que o turno desta sessão soube da estrutura (a API não guarda) */
export function serverTurnView(
  turn: AssistantTurn,
  context: TurnContext | null,
): TurnView {
  const attempts = [...turn.attempts].sort((a, b) => a.index - b.index);
  const shown = attempts.filter(attempt => attempt.output).at(-1) ?? null;
  const outcome = outcomeOf(turn.status);
  const execution = shown?.outcome?.execution;

  return {
    key: turn.id,
    turnId: turn.id,
    question: turn.question,
    live: false,
    phase: 'finished',
    outcome,
    attemptId: shown?.id ?? null,
    output: shown?.output ? toDisplay(shown.output) : null,
    streaming: false,
    failure:
      shown && outcome !== 'answered' && outcome !== 'cancelled'
        ? attemptFailure(shown)
        : null,
    failedAttempts: attempts
      .filter(attempt => attempt.id !== shown?.id)
      .map(attempt => ({
        attemptId: attempt.id,
        index: attempt.index,
        query: attempt.output?.query ?? null,
        failure: attemptFailure(attempt),
      })),
    truncated: context?.truncated ?? null,
    missingMentions: context?.missingMentions ?? [],
    shortenedMentions: context?.shortenedMentions ?? [],
    executedInTurn: execution?.ran === true && execution.ok === true,
    recordedRowCount:
      execution?.ran === true && execution.ok === true
        ? (shown?.outcome?.rowCount ?? null)
        : null,
    errorCode:
      outcome === 'failed'
        ? (ATTEMPT_STATUS_ERROR[attempts.at(-1)?.status ?? 'unknown'] ?? null)
        : null,
    errorMessage:
      turn.status === 'open' || turn.status === 'unknown'
        ? 'O turno não terminou.'
        : null,
    waitUntil: null,
    feedback: turn.feedback,
  };
}
