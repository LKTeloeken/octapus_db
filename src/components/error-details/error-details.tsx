import {
  Alert02Icon,
  Copy01Icon,
  Tick02Icon,
} from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import type { ReactNode } from 'react';
import { Button } from '@/components/ui/button';
import type { ErrorDetails as Details } from '@/lib/error-details';
import { cn } from '@/lib/utils';
import { useErrorDetails } from './use-error-details';

interface ErrorDetailsProps {
  details: Details;
  /** Ações além do copiar (tentar de novo, editar o servidor) */
  actions?: ReactNode;
  className?: string;
}

/**
 * Uma falha com o motivo: título pelo código do erro e a mensagem inteira,
 * selecionável, como o backend mandou (com as causas encadeadas). Usado no
 * "Testar conexão" do formulário e nos detalhes de erro da árvore.
 */
export function ErrorDetails({
  details,
  actions,
  className,
}: ErrorDetailsProps) {
  const { copied, copy } = useErrorDetails(details);

  return (
    <div
      role="alert"
      className={cn(
        'flex flex-col gap-2 rounded-md bg-danger-soft p-3',
        className,
      )}
    >
      <div className="flex items-center gap-2">
        <HugeiconsIcon
          icon={Alert02Icon}
          className="size-4 shrink-0 text-danger"
        />
        <span className="text-body font-medium text-fg">{details.title}</span>
      </div>
      <p className="max-h-40 overflow-y-auto font-mono text-small break-words whitespace-pre-wrap text-fg select-text scrollbar-thin">
        {details.message}
      </p>
      <div className="flex flex-wrap items-center gap-1.5">
        <Button variant="ghost" size="xs" onClick={() => void copy()}>
          <HugeiconsIcon icon={copied ? Tick02Icon : Copy01Icon} />
          {copied ? 'Copiado' : 'Copiar erro'}
        </Button>
        {actions}
      </div>
    </div>
  );
}
