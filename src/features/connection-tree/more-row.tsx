import { Spinner } from '@/components/ui/spinner';
import { formatCount } from '@/lib/format-count';
import { cn } from '@/lib/utils';

interface MoreRowProps {
  level: number;
  remaining: number;
  isLoading: boolean;
  isFocused: boolean;
  onMore: () => void;
}

/** A janela mostra parte dos filhos: o resto vem sob pedido, sem travar a árvore. */
export const MoreRow = ({
  level,
  remaining,
  isLoading,
  isFocused,
  onMore,
}: MoreRowProps) => (
  <button
    type="button"
    tabIndex={-1}
    onClick={onMore}
    disabled={isLoading}
    className={cn(
      'flex h-7 w-full cursor-pointer items-center gap-1.5 rounded-sm pr-2 text-small text-fg-muted transition-colors hover:bg-hover hover:text-fg disabled:cursor-default',
      isFocused && 'ring-[1.5px] ring-inset ring-ring',
    )}
    style={{ paddingLeft: `${6 + level * 14}px` }}
  >
    <span className="flex size-3.5 shrink-0 items-center justify-center">
      {isLoading && <Spinner className="size-3 text-fg-subtle" />}
    </span>
    <span className="truncate">Carregar mais</span>
    <span className="shrink-0 tabular-nums text-fg-subtle">
      {formatCount(remaining)} restantes
    </span>
  </button>
);
