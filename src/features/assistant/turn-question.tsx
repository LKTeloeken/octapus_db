import { useId } from 'react';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import type { QuestionView } from './turn-view';
import { useTurnQuestion } from './use-turn-question';

interface TurnQuestionProps {
  question: QuestionView;
  /** O turno está parado esperando esta resposta */
  waiting: boolean;
  /** O turno terminou: pergunta sem resposta fica marcada */
  finished: boolean;
  clientTurnId: string | null;
}

/**
 * Uma pergunta do assistente no meio do turno (`ask_user`) e a resposta do
 * usuário. Enquanto o turno espera, as opções e o campo para responder.
 */
export function TurnQuestion({
  question,
  waiting,
  finished,
  clientTurnId,
}: TurnQuestionProps) {
  const reply = useTurnQuestion({ clientTurnId });
  const fieldId = useId();
  const countId = useId();

  return (
    <div className="flex flex-col gap-2">
      <p className="text-body whitespace-pre-wrap break-words text-fg">
        {question.question}
      </p>

      {question.reply !== null && (
        <div className="self-end max-w-[88%] rounded-lg bg-active px-3 py-2 text-body whitespace-pre-wrap break-words text-fg">
          {question.reply}
        </div>
      )}

      {question.reply === null && waiting && (
        <form
          aria-label="Responder ao assistente"
          className="flex flex-col gap-2"
          onSubmit={event => {
            event.preventDefault();
            reply.sendDraft();
          }}
        >
          {question.choices.length > 0 && (
            <div
              role="group"
              aria-label="Respostas sugeridas"
              className="flex flex-wrap gap-1.5"
            >
              {question.choices.map(choice => (
                <Button
                  key={choice}
                  type="button"
                  variant="outline"
                  size="xs"
                  className="rounded-full"
                  disabled={reply.sending}
                  onClick={() => reply.choose(choice)}
                >
                  {choice}
                </Button>
              ))}
            </div>
          )}
          <label htmlFor={fieldId} className="sr-only">
            Sua resposta
          </label>
          <textarea
            id={fieldId}
            rows={2}
            value={reply.draft}
            placeholder={
              question.choices.length > 0
                ? 'Ou escreva outra resposta…'
                : 'Sua resposta…'
            }
            onChange={event => reply.setDraft(event.target.value)}
            onKeyDown={event => {
              // Enter responde; Shift+Enter quebra a linha
              if (
                event.key === 'Enter' &&
                !event.shiftKey &&
                !event.nativeEvent.isComposing
              ) {
                event.preventDefault();
                reply.sendDraft();
              }
            }}
            aria-describedby={countId}
            aria-invalid={reply.tooLong || undefined}
            className={cn(
              'w-full resize-y rounded-sm border border-line bg-field p-2 text-body text-fg outline-none placeholder:text-fg-subtle focus:border-iris-text focus:ring-3 focus:ring-iris-soft',
              reply.tooLong &&
                'border-danger focus:border-danger focus:ring-danger-soft',
            )}
          />
          <div className="flex items-center gap-3">
            <span
              id={countId}
              className={cn(
                'min-w-0 flex-1 text-small',
                reply.tooLong ? 'text-danger' : 'text-fg-subtle',
              )}
            >
              {reply.tooLong
                ? `A resposta passou de ${reply.max} caracteres.`
                : 'O assistente continua com a sua resposta.'}
            </span>
            <Button type="submit" size="xs" disabled={!reply.canSend}>
              Responder
            </Button>
          </div>
        </form>
      )}

      {question.reply === null && !waiting && finished && (
        <p className="text-small text-fg-subtle">Sem resposta.</p>
      )}
    </div>
  );
}
