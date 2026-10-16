import { Spinner } from '@/components/ui/spinner';
import { cn } from '@/lib/utils';

interface CommandPreviewProps {
  command: string | null;
  error: string | null;
  isLoading: boolean;
}

/**
 * A linha de comando que vai rodar, montada pelo backend (a mesma que ele
 * executa, sem a senha). Uma combinação inválida aparece aqui em vermelho.
 */
export function CommandPreview({
  command,
  error,
  isLoading,
}: CommandPreviewProps) {
  return (
    <div className="flex flex-col gap-1.5">
      <div className="flex h-4 items-center gap-2 text-small font-medium text-fg-muted">
        Comando
        {isLoading && <Spinner className="size-3 text-fg-subtle" />}
      </div>
      <pre
        className={cn(
          'max-h-32 min-h-12 overflow-auto rounded-sm border border-line bg-field px-2.5 py-2 font-mono text-small whitespace-pre-wrap break-all scrollbar-thin',
          error ? 'text-danger' : 'text-fg-muted',
        )}
      >
        {error ?? command ?? ' '}
      </pre>
    </div>
  );
}
