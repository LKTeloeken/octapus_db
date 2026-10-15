import { Spinner } from '@/components/ui/spinner';
import { cn } from '@/lib/utils';
import type { SqlPreviewProps } from './structure-editor.types';

/**
 * O SQL que vai rodar, montado pelo backend — o mesmo que ele aplica. Erro de
 * validação (tipo inválido, default com mais de uma expressão) aparece aqui,
 * antes de qualquer coisa ir ao banco.
 */
export function SqlPreview({
  statements,
  error,
  isLoading,
  emptyLabel = 'Nada para alterar.',
}: SqlPreviewProps) {
  const text = statements?.map(statement => `${statement};`).join('\n\n');

  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex h-4 items-center gap-2 text-small font-medium text-fg-muted">
        SQL
        {isLoading && <Spinner className="size-3 text-fg-subtle" />}
      </div>
      <pre
        className={cn(
          'max-h-44 min-h-12 overflow-auto rounded-sm border border-line bg-field px-2.5 py-2 font-mono text-small whitespace-pre-wrap break-words scrollbar-thin',
          error ? 'text-danger' : 'text-fg-muted',
        )}
      >
        {error ?? (text || (statements ? emptyLabel : ' '))}
      </pre>
    </div>
  );
}
