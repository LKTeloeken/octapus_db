import {
  Copy01Icon,
  PlayIcon,
  SourceCodeIcon,
} from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { QueryEditor } from '@/components/query-editor/query-editor/query-editor';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { Tooltip } from '@/components/ui/tooltip/tooltip';
import type { ReadOnlyPlan } from '@/api/types/assistant.types';
import type { QueryState } from './use-turn-card';

const STATE_BADGE: Record<
  QueryState,
  { label: string; variant: 'success' | 'neutral' | 'danger' | 'warning' }
> = {
  executed: { label: 'Executada', variant: 'success' },
  'executed-earlier': { label: 'Executada no turno', variant: 'success' },
  executing: { label: 'Executando…', variant: 'neutral' },
  review: { label: 'Não executada', variant: 'neutral' },
  failed: { label: 'Não executada', variant: 'danger' },
  'not-executed': { label: 'Não executada', variant: 'neutral' },
};

const numberFormat = new Intl.NumberFormat('pt-BR', {
  maximumFractionDigits: 1,
});

interface QueryBlockProps {
  /** `code` = código que nunca é executado (RN-14) */
  kind: 'query' | 'code';
  text: string;
  onChange?: (text: string) => void;
  /** Editável só na revisão (RN-13) */
  editable: boolean;
  streaming: boolean;
  state: QueryState | null;
  plan: ReadOnlyPlan | null;
  edited: boolean;
  onExecute?: () => void;
  canExecute: boolean;
  onOpenInEditor: () => void;
  canOpenInEditor: boolean;
  onCopy: () => void;
}

/**
 * A consulta (ou o código) da resposta, sempre visível (RN-18), com destaque
 * de sintaxe. O selo diz se ela rodou — nunca "executada" sem ter rodado.
 */
export function QueryBlock({
  kind,
  text,
  onChange,
  editable,
  streaming,
  state,
  plan,
  edited,
  onExecute,
  canExecute,
  onOpenInEditor,
  canOpenInEditor,
  onCopy,
}: QueryBlockProps) {
  const badge = kind === 'code' ? null : state ? STATE_BADGE[state] : null;

  return (
    <div className="overflow-hidden rounded-md border border-line">
      <div className="flex h-8 items-center gap-2 border-b border-line-subtle bg-surface-2 pr-1 pl-3">
        <span className="text-small font-medium text-fg-muted">
          {kind === 'code' ? 'Código' : 'Consulta'}
        </span>
        {badge && <Badge variant={badge.variant}>{badge.label}</Badge>}
        {edited && <Badge>Editada</Badge>}
        <span className="flex-1" />
        <Tooltip content="Copiar" position="top">
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label="Copiar"
            onClick={onCopy}
            disabled={!text || streaming}
          >
            <HugeiconsIcon icon={Copy01Icon} />
          </Button>
        </Tooltip>
        <Tooltip content="Abrir no editor" position="top">
          <Button
            variant="ghost"
            size="icon-xs"
            aria-label="Abrir no editor"
            onClick={onOpenInEditor}
            disabled={!canOpenInEditor || !text || streaming}
          >
            <HugeiconsIcon icon={SourceCodeIcon} />
          </Button>
        </Tooltip>
      </div>

      <QueryEditor
        value={text}
        onChange={onChange ?? (() => {})}
        dialect="postgres"
        readOnly={!editable}
        height="auto"
        maxHeight="240px"
        placeholderText=""
        fontSize={12}
      />

      {state === 'review' && onExecute && (
        <div className="flex items-center gap-3 border-t border-line-subtle bg-surface-2 px-3 py-1.5">
          <span className="min-w-0 flex-1 truncate text-small text-fg-muted">
            {plan?.estimatedRows != null
              ? `Estimativa: ~${numberFormat.format(plan.estimatedRows)} linha(s)` +
                (plan.estimatedCost != null
                  ? `, custo ${numberFormat.format(plan.estimatedCost)}`
                  : '')
              : 'Revise a consulta antes de executar.'}
          </span>
          <Button size="xs" onClick={onExecute} disabled={!canExecute}>
            <HugeiconsIcon icon={PlayIcon} />
            Executar
          </Button>
        </div>
      )}
    </div>
  );
}
