import { memo } from 'react';
import { Input } from '@/components/ui/input/input';
import type { FilterInputProps } from './filter-input.types';

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
        inputClassName="font-mono"
        InputProps={{
          startAdornment: (
            <span className="font-mono text-muted-foreground">WHERE</span>
          ),
        }}
      />
    );
  },
);

FilterInput.displayName = 'FilterInput';
