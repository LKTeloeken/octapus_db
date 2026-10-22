import {
  Alert02Icon,
  Edit01Icon,
  Refresh01Icon,
} from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { useMemo, useState } from 'react';
import { ErrorDetails } from '@/components/error-details/error-details';
import { Button } from '@/components/ui/button';
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover';
import { errorDetails } from '@/lib/error-details';

interface ErrorRowProps {
  level: number;
  message: string;
  /** O erro em si: com ele, os detalhes ganham o título pelo código */
  error?: unknown;
  onRetry: () => void;
  /** Falha de conexão: "Editar servidor" nos detalhes */
  onEdit?: () => void;
}

const LINK =
  'shrink-0 cursor-pointer rounded-xs text-fg-muted underline underline-offset-2 transition-colors outline-none hover:text-fg focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-offset-2 focus-visible:outline-ring';

/**
 * Falha ao carregar um nível da árvore: mensagem em `--danger`, os detalhes
 * (o motivo inteiro, copiável, com "Editar servidor" quando é conexão) e nova
 * tentativa.
 */
export const ErrorRow = ({
  level,
  message,
  error,
  onRetry,
  onEdit,
}: ErrorRowProps) => {
  const [open, setOpen] = useState(false);
  const details = useMemo(
    () => errorDetails(error ?? message),
    [error, message],
  );

  const run = (action: () => void) => () => {
    setOpen(false);
    action();
  };

  return (
    <div
      className="flex h-7 items-center gap-1.5 pr-2 text-small text-danger"
      style={{ paddingLeft: `${6 + level * 14}px` }}
    >
      <span className="flex size-3.5 shrink-0" />
      <HugeiconsIcon icon={Alert02Icon} className="size-4 shrink-0" />
      <span className="min-w-0 truncate" title={message}>
        Falha ao carregar
      </span>
      <Popover open={open} onOpenChange={setOpen}>
        <PopoverTrigger asChild>
          <button type="button" className={LINK}>
            detalhes
          </button>
        </PopoverTrigger>
        <PopoverContent className="w-96 p-1.5" align="start">
          <ErrorDetails
            details={details}
            actions={
              <>
                <Button variant="ghost" size="xs" onClick={run(onRetry)}>
                  <HugeiconsIcon icon={Refresh01Icon} />
                  Tentar de novo
                </Button>
                {onEdit && (
                  <Button variant="ghost" size="xs" onClick={run(onEdit)}>
                    <HugeiconsIcon icon={Edit01Icon} />
                    Editar servidor
                  </Button>
                )}
              </>
            }
          />
        </PopoverContent>
      </Popover>
      {/* Só o ícone: a linha é estreita nos níveis fundos, e o texto da falha
          vem primeiro */}
      <button
        type="button"
        aria-label="Tentar de novo"
        title="Tentar de novo"
        className="flex size-5 shrink-0 cursor-pointer items-center justify-center rounded-xs text-fg-muted transition-colors outline-none hover:bg-hover hover:text-fg focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-offset-2 focus-visible:outline-ring"
        onClick={onRetry}
      >
        <HugeiconsIcon icon={Refresh01Icon} className="size-3.5" />
      </button>
    </div>
  );
};
