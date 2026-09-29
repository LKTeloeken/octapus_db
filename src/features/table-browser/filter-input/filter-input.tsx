import { memo } from 'react';
import { Input } from '@/components/ui/input/input';
import { Kbd } from '@/components/ui/kbd';
import type { FilterInputProps } from './filter-input.types';

/** WHERE do Postgres em mono 12 px; Enter aplica, Esc volta ao filtro aplicado. */
export const FilterInput = memo(
  ({ value, onChange, onApply, onReset }: FilterInputProps) => {
    return (
      <Input
        size="sm"
        className="flex-1 min-w-0"
        placeholder="id = 1 AND name ILIKE '%foo%'"
        value={value}
        onChange={event => onChange(event.target.value)}
        onKeyDown={event => {
          if (event.key === 'Enter') {
            event.preventDefault();
            onApply();
          }
          if (event.key === 'Escape') {
            event.preventDefault();
            onReset();
          }
        }}
        inputClassName="font-mono text-small"
        InputProps={{
          startAdornment: (
            <span className="font-mono text-[11px] tracking-[0.04em] text-fg-subtle">
              WHERE
            </span>
          ),
          endAdornment: <Kbd>↵</Kbd>,
        }}
      />
    );
  },
);

FilterInput.displayName = 'FilterInput';
