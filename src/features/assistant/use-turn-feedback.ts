import { useState } from 'react';
import toast from 'react-hot-toast';
import { ApiError } from '@/api/client';
import type {
  AssistantFeedback,
  FeedbackRating,
} from '@/api/types/assistant.types';
import {
  FEEDBACK_COMMENT_MAX,
  FEEDBACK_REASONS,
  commentLength,
  nextRating,
  toggleReason,
  withComment,
} from '@/lib/assistant/feedback';
import {
  useTurnFeedbackMutation,
  type FeedbackChange,
} from '@/queries/use-assistant-feedback';
import { feedbackErrorMessage } from './turn-messages';

export type ShownReason = (typeof FEEDBACK_REASONS)[number];

export const REASON_LABELS: Record<ShownReason, string> = {
  wrong_result: 'Resultado errado',
  wrong_query: 'Consulta errada',
  did_not_understand: 'Não entendeu a pergunta',
  unclear_explanation: 'Explicação confusa',
  slow: 'Demorou demais',
  other: 'Outro motivo',
};

/**
 * A avaliação de um turno respondido (B11). 👍/👎 e os motivos gravam no
 * clique (otimista: a tela muda na hora e volta se o servidor recusar); o
 * comentário é um rascunho até o Salvar. O rascunho vale enquanto o
 * comentário salvo for o mesmo de quando ele começou: salvar mostra o salvo,
 * e uma recusa devolve o rascunho para tentar de novo. Remover a avaliação
 * descarta o rascunho.
 */
export const useTurnFeedback = ({
  conversationId,
  turnId,
  attemptId,
  feedback,
}: {
  conversationId: string;
  turnId: string;
  attemptId: string | null;
  feedback: AssistantFeedback | null;
}) => {
  const mutation = useTurnFeedbackMutation(conversationId, turnId);
  const saved = feedback?.comment ?? '';
  const [draft, setDraft] = useState({ base: saved, text: saved });
  const comment = draft.base === saved ? draft.text : saved;
  // No 👍 o comentário fica recolhido até a pessoa pedir
  const [commentRequested, setCommentRequested] = useState(false);

  const save = (change: FeedbackChange) =>
    mutation.mutate(change, {
      onError: error =>
        toast.error(
          feedbackErrorMessage(error instanceof ApiError ? error.code : null),
        ),
    });

  const rating = feedback?.rating ?? null;
  const length = commentLength(comment);
  const tooLong = length > FEEDBACK_COMMENT_MAX;
  const dirty = comment.trim() !== saved.trim();

  return {
    rating,
    reasons: FEEDBACK_REASONS.map(reason => ({
      reason,
      label: REASON_LABELS[reason],
      selected: feedback?.reasons.includes(reason) ?? false,
    })),
    rate: (clicked: FeedbackRating) => {
      const change = nextRating(feedback, clicked, attemptId);
      if (!change) {
        setDraft({ base: '', text: '' });
        setCommentRequested(false);
      }
      save(change);
    },
    toggleReason: (reason: ShownReason) => {
      const change = toggleReason(feedback, reason, attemptId);
      if (change) save(change);
    },
    showComment: rating === 'down' || (rating === 'up' && commentRequested),
    canRequestComment: rating === 'up' && !commentRequested,
    requestComment: () => setCommentRequested(true),
    hasSavedComment: saved !== '',
    comment,
    setComment: (text: string) => setDraft({ base: saved, text }),
    commentLength: length,
    commentMax: FEEDBACK_COMMENT_MAX,
    commentTooLong: tooLong,
    canSaveComment: !!feedback && dirty && !tooLong,
    saveComment: () => {
      if (feedback && dirty && !tooLong) {
        save(withComment(feedback, comment, attemptId));
      }
    },
  };
};
