import {
  ThumbsDownIcon,
  ThumbsUpIcon,
  Tick01Icon,
} from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { useId } from 'react';
import type { AssistantFeedback } from '@/api/types/assistant.types';
import { Button } from '@/components/ui/button';
import { Tooltip } from '@/components/ui/tooltip/tooltip';
import { cn } from '@/lib/utils';
import { useTurnFeedback } from './use-turn-feedback';

interface TurnFeedbackProps {
  conversationId: string;
  turnId: string;
  attemptId: string | null;
  feedback: AssistantFeedback | null;
}

/** 👍/👎 abaixo da resposta; no 👎, motivos e comentário (B11) */
export function TurnFeedback(props: TurnFeedbackProps) {
  const fb = useTurnFeedback(props);
  const commentId = useId();
  const hintId = useId();
  const countId = useId();

  return (
    <div
      role="group"
      aria-label="Avaliação da resposta"
      className="flex flex-col gap-2 border-t border-line-subtle pt-2"
    >
      <div className="flex items-center gap-1">
        <span className="mr-1 text-small text-fg-subtle">
          {fb.rating ? 'Obrigado pela avaliação.' : 'A resposta ajudou?'}
        </span>
        <Tooltip content={fb.rating === 'up' ? 'Remover avaliação' : 'Ajudou'}>
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label="Ajudou"
            aria-pressed={fb.rating === 'up'}
            onClick={() => fb.rate('up')}
          >
            <HugeiconsIcon icon={ThumbsUpIcon} strokeWidth={1.5} />
          </Button>
        </Tooltip>
        <Tooltip
          content={fb.rating === 'down' ? 'Remover avaliação' : 'Não ajudou'}
        >
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label="Não ajudou"
            aria-pressed={fb.rating === 'down'}
            onClick={() => fb.rate('down')}
          >
            <HugeiconsIcon icon={ThumbsDownIcon} strokeWidth={1.5} />
          </Button>
        </Tooltip>
        {fb.canRequestComment && (
          <Button variant="link" size="xs" onClick={fb.requestComment}>
            {fb.hasSavedComment ? 'Ver comentário' : 'Comentar'}
          </Button>
        )}
      </div>

      {fb.rating === 'down' && (
        <div
          role="group"
          aria-label="O que deu errado?"
          className="flex flex-wrap gap-1.5"
        >
          {fb.reasons.map(({ reason, label, selected }) => (
            <Button
              key={reason}
              variant="outline"
              size="xs"
              aria-pressed={selected}
              onClick={() => fb.toggleReason(reason)}
              className="rounded-full aria-pressed:border-iris-text aria-pressed:bg-iris-soft"
            >
              {selected && (
                <HugeiconsIcon icon={Tick01Icon} strokeWidth={1.5} />
              )}
              {label}
            </Button>
          ))}
        </div>
      )}

      {fb.showComment && (
        <div className="flex flex-col gap-1.5">
          <div className="flex items-center gap-3">
            <label htmlFor={commentId} className="text-small text-fg-muted">
              Comentário (opcional)
            </label>
            <span className="flex-1" />
            <span
              id={countId}
              className={cn(
                'text-small tabular-nums',
                fb.commentTooLong ? 'text-danger' : 'text-fg-subtle',
              )}
            >
              {fb.commentLength}/{fb.commentMax}
            </span>
          </div>
          <textarea
            id={commentId}
            rows={2}
            value={fb.comment}
            onChange={event => fb.setComment(event.target.value)}
            aria-describedby={`${hintId} ${countId}`}
            aria-invalid={fb.commentTooLong || undefined}
            className={cn(
              'w-full resize-y rounded-sm border border-line bg-field p-2 text-small text-fg outline-none placeholder:text-fg-subtle focus:border-iris-text focus:ring-3 focus:ring-iris-soft',
              fb.commentTooLong &&
                'border-danger focus:border-danger focus:ring-danger-soft',
            )}
          />
          <div className="flex items-center gap-3">
            <span
              id={hintId}
              className="min-w-0 flex-1 text-small text-fg-subtle"
            >
              Não cole dados do seu banco aqui.
            </span>
            <Button
              variant="outline"
              size="xs"
              onClick={fb.saveComment}
              disabled={!fb.canSaveComment}
            >
              Salvar comentário
            </Button>
          </div>
          {fb.commentTooLong && (
            <span role="alert" className="text-small text-danger">
              O comentário passou de {fb.commentMax} caracteres.
            </span>
          )}
        </div>
      )}
    </div>
  );
}
