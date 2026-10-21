import type {
  AssistantConversation,
  AssistantFeedback,
  AssistantFeedbackInput,
  FeedbackRating,
  FeedbackReason,
} from '@/api/types/assistant.types';

/**
 * A avaliação de um turno (👍/👎), em funções puras: o espelho do
 * `AssistantFeedbackInput::normalized` do Rust (para a atualização otimista e
 * o mock) e as transições dos botões.
 */

/** Teto do comentário, em caracteres (o mesmo do Rust e da API) */
export const FEEDBACK_COMMENT_MAX = 2000;

/** Os motivos do 👎, na ordem da tela (`unknown` = valor de uma API mais nova) */
export const FEEDBACK_REASONS = [
  'wrong_result',
  'wrong_query',
  'did_not_understand',
  'unclear_explanation',
  'slow',
  'other',
] as const satisfies readonly FeedbackReason[];

/** Caracteres como a API conta (runes): um emoji é um, não dois */
export const commentLength = (comment: string) =>
  Array.from(comment.trim()).length;

export const commentTooLong = (comment: string) =>
  commentLength(comment) > FEEDBACK_COMMENT_MAX;

/**
 * O que a API aceita: motivos só no 👎, sem repetir e sem os desconhecidos;
 * comentário aparado, vazio vira `null`.
 */
export function normalizeFeedback(
  input: AssistantFeedbackInput,
): AssistantFeedbackInput {
  const reasons =
    input.rating === 'down'
      ? Array.from(new Set(input.reasons)).filter(
          reason => reason !== 'unknown',
        )
      : [];
  const comment = input.comment?.trim() || null;
  return { ...input, reasons, comment };
}

/** O que a avaliação passa a ser, antes de o servidor responder */
export function optimisticFeedback(
  input: AssistantFeedbackInput,
  previous: AssistantFeedback | null,
  now: string,
): AssistantFeedback {
  const normalized = normalizeFeedback(input);
  return {
    rating: normalized.rating,
    reasons: normalized.reasons,
    comment: normalized.comment,
    attemptId: normalized.attemptId,
    // Como na API: a primeira avaliação marca o `createdAt`
    createdAt: previous?.createdAt ?? now,
    updatedAt: now,
  };
}

/** A conversa com a avaliação do turno trocada (cópia; o resto fica igual) */
export function withTurnFeedback(
  conversation: AssistantConversation,
  turnId: string,
  feedback: AssistantFeedback | null,
): AssistantConversation {
  return {
    ...conversation,
    turns: conversation.turns.map(turn =>
      turn.id === turnId ? { ...turn, feedback } : turn,
    ),
  };
}

export function turnFeedbackOf(
  conversation: AssistantConversation | undefined,
  turnId: string,
): AssistantFeedback | null {
  return conversation?.turns.find(turn => turn.id === turnId)?.feedback ?? null;
}

/**
 * Clicar em 👍 ou 👎: o mesmo botão de novo remove a avaliação (`null`); o
 * outro troca, levando o comentário (os motivos são só do 👎).
 */
export function nextRating(
  current: AssistantFeedback | null,
  clicked: FeedbackRating,
  attemptId: string | null,
): AssistantFeedbackInput | null {
  if (current?.rating === clicked) return null;
  return {
    rating: clicked,
    reasons: [],
    comment: current?.comment ?? null,
    attemptId,
  };
}

/** Liga ou desliga um motivo do 👎 (sem 👎, nada a fazer) */
export function toggleReason(
  current: AssistantFeedback | null,
  reason: FeedbackReason,
  attemptId: string | null,
): AssistantFeedbackInput | null {
  if (current?.rating !== 'down') return null;
  const reasons = current.reasons.includes(reason)
    ? current.reasons.filter(item => item !== reason)
    : [...current.reasons, reason];
  return {
    rating: 'down',
    reasons,
    comment: current.comment,
    attemptId,
  };
}

/** Grava o comentário na avaliação atual */
export function withComment(
  current: AssistantFeedback,
  comment: string,
  attemptId: string | null,
): AssistantFeedbackInput {
  return {
    rating: current.rating,
    reasons: current.reasons,
    comment,
    attemptId,
  };
}
