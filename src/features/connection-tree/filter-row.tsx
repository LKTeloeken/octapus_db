import { FilterHorizontalIcon } from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import type { KeyboardEvent } from 'react';
import { Input } from '@/components/ui/input/input';
import { formatCount } from '@/lib/format-count';

interface FilterRowProps {
  level: number;
  /** Nó dono do filtro — o `/` da árvore acha o campo por ele */
  nodeId: string;
  value: string;
  total: number;
  placeholder: string;
  onChange: (value: string) => void;
  /** Devolve o teclado para a árvore (Enter, ↓, Esc com o campo vazio) */
  onLeave: () => void;
}

/**
 * Filtro dos filhos de um nó grande (milhares de schemas de tenant). O texto
 * vive no `tree-store`: a linha sai do DOM ao rolar para fora da lista
 * virtualizada, e o filtro não pode se perder com ela.
 */
export const FilterRow = ({
  level,
  nodeId,
  value,
  total,
  placeholder,
  onChange,
  onLeave,
}: FilterRowProps) => {
  const onKeyDown = (event: KeyboardEvent<HTMLInputElement>) => {
    // As setas e o Enter são da árvore, não do campo
    event.stopPropagation();
    if (event.key === 'Enter' || event.key === 'ArrowDown') {
      event.preventDefault();
      onLeave();
    } else if (event.key === 'Escape') {
      event.preventDefault();
      if (value) onChange('');
      else onLeave();
    }
  };

  return (
    <div
      className="flex h-7 items-center pr-1"
      style={{ paddingLeft: `${6 + level * 14}px` }}
    >
      <Input
        size="sm"
        aria-label={placeholder}
        placeholder={placeholder}
        value={value}
        onChange={event => onChange(event.target.value)}
        onKeyDown={onKeyDown}
        data-tree-filter={nodeId}
        controlClassName="h-6 gap-1 px-1.5 text-small"
        inputClassName="text-small"
        InputProps={{
          startAdornment: (
            <HugeiconsIcon icon={FilterHorizontalIcon} className="size-3.5" />
          ),
          endAdornment: (
            <span className="font-mono text-[11px] tabular-nums text-fg-subtle">
              {formatCount(total)}
            </span>
          ),
        }}
      />
    </div>
  );
};
