import { Alert02Icon, InformationCircleIcon } from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import type { ReactNode } from 'react';
import { OctapusAnimated } from '@/components/octapus-animated/octapus-animated';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import type { AssistantTarget } from '@/stores/assistant-store';
import { QuestionText } from './question-text';
import { QueryBlock } from './query-block';
import { ResultBlock } from './result-block';
import { STAGE_LABELS, phaseLabel, turnErrorMessage } from './turn-messages';
import { useCountdown } from './use-countdown';
import type { ConversationTurn } from './use-conversation-view';
import { useTurnCard } from './use-turn-card';
import { useTurnMascot } from './use-turn-mascot';

interface TurnCardProps {
  turn: ConversationTurn;
  target: AssistantTarget | null;
  flat: boolean;
  onCancel: () => void;
  isCancelling: boolean;
}

/** Uma pergunta e a resposta: explicação, premissas, consulta e resultado */
export function TurnCard({
  turn,
  target,
  flat,
  onCancel,
  isCancelling,
}: TurnCardProps) {
  const card = useTurnCard({ turn, target, flat });
  const { view, run } = card;
  const output = view.output;
  const running = view.outcome === 'running';
  const seconds = useCountdown(view.waitUntil);
  const correcting = running && view.failedAttempts.length > 0;
  const mascot = useTurnMascot(view);

  return (
    <article className="flex flex-col gap-3">
      <div className="self-end max-w-[88%] rounded-lg bg-active px-3 py-2 text-body whitespace-pre-wrap break-words text-fg">
        <QuestionText text={view.question} flat={flat} />
      </div>

      {view.truncated && (
        <Notice tone="info">
          A estrutura do banco é grande: o assistente viu só um recorte das
          tabelas. Cite as tabelas com @ para garantir que entrem.
        </Notice>
      )}

      {mascot && (
        <div className="flex items-center gap-2.5">
          <OctapusAnimated
            scene={mascot}
            className={output ? 'size-8' : 'size-12'}
          />
          {running && (
            <>
              <span className="text-small text-fg-muted" role="status">
                {view.phase === 'waiting' && seconds !== null
                  ? `Aguardando a vez… ${seconds} s`
                  : phaseLabel(view.phase, view.streaming, correcting)}
              </span>
              <span className="flex-1" />
              <Button
                variant="ghost"
                size="xs"
                onClick={onCancel}
                disabled={isCancelling}
              >
                Cancelar
              </Button>
            </>
          )}
        </div>
      )}

      {output?.clarificationQuestion && (
        <p className="text-body whitespace-pre-wrap text-fg">
          {output.clarificationQuestion}
        </p>
      )}

      {output?.explanation && (
        <p className="text-body whitespace-pre-wrap text-fg">
          {output.explanation}
        </p>
      )}

      {output && output.assumptions.length > 0 && (
        <div className="flex flex-col gap-1">
          <span className="text-micro text-fg-subtle uppercase">Premissas</span>
          <ul className="flex list-disc flex-col gap-0.5 pl-5 text-small text-fg-muted">
            {output.assumptions.map((assumption, index) => (
              <li key={index}>{assumption}</li>
            ))}
          </ul>
        </div>
      )}

      {output?.kind === 'code' && (
        <Notice tone="warning">
          Código não executado. O assistente nunca executa código que não seja
          consulta de leitura — se você rodar, a execução é sua. Revise antes.
        </Notice>
      )}

      {output?.query != null && output.kind !== 'clarification' && (
        <QueryBlock
          kind={output.kind === 'code' ? 'code' : 'query'}
          text={card.isQuery ? card.draft : output.query}
          onChange={card.setDraft}
          editable={card.queryState === 'review'}
          streaming={view.streaming}
          state={card.isQuery ? card.queryState : null}
          plan={run?.plan ?? null}
          edited={card.isQuery && card.edited}
          onExecute={() => void card.runQuery(card.draft)}
          canExecute={card.canExecute}
          onOpenInEditor={() =>
            card.openInEditor(card.isQuery ? card.draft : (output.query ?? ''))
          }
          canOpenInEditor={card.canOpenInEditor}
          onCopy={() =>
            void card.copy(card.isQuery ? card.draft : (output.query ?? ''))
          }
        />
      )}

      {view.failure && (
        <Notice tone="danger">
          {STAGE_LABELS[view.failure.stage]}
          {view.failure.message ? `: ${view.failure.message}` : '.'}
        </Notice>
      )}

      {view.outcome === 'exhausted' && (
        <Notice tone="warning">
          Não cheguei a uma consulta que funcionasse depois de{' '}
          {view.failedAttempts.length + 1} tentativa(s). Esta é a última que
          gerei — ela não foi executada. Abra no editor para ajustar.
        </Notice>
      )}

      {view.outcome === 'failed' && (
        <Notice tone="danger">
          {turnErrorMessage(view.errorCode, view.errorMessage)}
        </Notice>
      )}

      {view.outcome === 'cancelled' && (
        <p className="text-small text-fg-subtle">Cancelado.</p>
      )}

      {run?.error && <Notice tone="danger">{run.error}</Notice>}

      {run?.result && view.attemptId && (
        <ResultBlock
          attemptId={view.attemptId}
          result={run.result}
          loadingMore={run.loadingMore}
          onLoadMore={() => void card.loadMore()}
        />
      )}

      {!run?.result && card.queryState === 'executed-earlier' && (
        <div className="flex items-center gap-3 text-small text-fg-muted">
          <span>
            Executada no turno
            {view.recordedRowCount !== null
              ? ` (${view.recordedRowCount} linha(s))`
              : ''}
            . O resultado não fica guardado.
          </span>
          <Button
            variant="outline"
            size="xs"
            onClick={() => void card.runQuery(card.draft)}
            disabled={!card.canExecute}
          >
            Executar de novo
          </Button>
        </div>
      )}

      {view.failedAttempts.length > 0 && !running && (
        <details className="text-small text-fg-muted">
          <summary className="cursor-pointer select-none">
            {view.failedAttempts.length}{' '}
            {view.output
              ? 'tentativa(s) anterior(es) falharam'
              : 'tentativa(s) falharam'}
          </summary>
          <ol className="mt-2 flex flex-col gap-2">
            {view.failedAttempts.map(attempt => (
              <li key={attempt.attemptId} className="flex flex-col gap-1">
                {attempt.query && (
                  <code className="block rounded-sm bg-surface-2 px-2 py-1 font-mono text-small whitespace-pre-wrap text-fg-muted">
                    {attempt.query}
                  </code>
                )}
                <span className="text-danger">
                  {attempt.failure
                    ? `${STAGE_LABELS[attempt.failure.stage]}${attempt.failure.message ? `: ${attempt.failure.message}` : ''}`
                    : 'Falhou.'}
                </span>
              </li>
            ))}
          </ol>
        </details>
      )}

      {/* B11: a avaliação (👍/👎) entra aqui, abaixo da resposta */}
    </article>
  );
}

function Notice({
  tone,
  children,
}: {
  tone: 'info' | 'warning' | 'danger';
  children: ReactNode;
}) {
  const styles = {
    info: 'bg-info-soft text-fg',
    warning: 'bg-warning-soft text-fg',
    danger: 'bg-danger-soft text-fg',
  }[tone];
  const iconColor = {
    info: 'text-info',
    warning: 'text-warning',
    danger: 'text-danger',
  }[tone];

  return (
    <div className={cn('flex gap-2 rounded-md px-3 py-2 text-small', styles)}>
      <HugeiconsIcon
        icon={tone === 'info' ? InformationCircleIcon : Alert02Icon}
        className={cn('mt-0.5 size-3.5 shrink-0', iconColor)}
      />
      <div className="min-w-0 break-words">{children}</div>
    </div>
  );
}
