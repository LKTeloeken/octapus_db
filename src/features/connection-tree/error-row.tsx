import { Alert02Icon } from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';

interface ErrorRowProps {
  level: number;
  message: string;
  onRetry: () => void;
}

/** Falha ao carregar um nível da árvore: mensagem em `--danger` + nova tentativa. */
export const ErrorRow = ({ level, message, onRetry }: ErrorRowProps) => {
  return (
    <div
      className="flex h-7 items-center gap-1.5 pr-2 text-small text-danger"
      style={{ paddingLeft: `${6 + level * 14}px` }}
    >
      <span className="flex size-3.5 shrink-0" />
      <HugeiconsIcon icon={Alert02Icon} className="size-4 shrink-0" />
      <span className="truncate" title={message}>
        Falha ao carregar
      </span>
      <button
        type="button"
        className="shrink-0 cursor-pointer rounded-xs text-fg-muted underline underline-offset-2 transition-colors outline-none hover:text-fg focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-offset-2 focus-visible:outline-ring"
        onClick={onRetry}
      >
        tentar de novo
      </button>
    </div>
  );
};
