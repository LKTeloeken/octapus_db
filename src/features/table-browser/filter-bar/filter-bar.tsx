import {
  Add01Icon,
  Cancel01Icon,
  FilterIcon,
} from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { memo, useState } from 'react';
import { Button } from '@/components/ui/button';
import { Input } from '@/components/ui/input/input';
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover';
import {
  Select,
  SelectContent,
  SelectItem,
  SelectTrigger,
  SelectValue,
} from '@/components/ui/select';
import type { ColumnFilter, FilterOp } from '@/api/types/browse.types';
import type { FilterBarProps } from './filter-bar.types';

const OP_LABELS: Record<FilterOp, string> = {
  eq: '=',
  ne: '≠',
  gt: '>',
  gte: '≥',
  lt: '<',
  lte: '≤',
  like: 'like',
  in: 'in',
  is_null: 'é null',
  not_null: 'não é null',
};

const VALUELESS_OPS: FilterOp[] = ['is_null', 'not_null'];

export const FilterBar = memo(
  ({ columns, filters, onChange }: FilterBarProps) => {
    const [isOpen, setIsOpen] = useState(false);
    const [column, setColumn] = useState('');
    const [op, setOp] = useState<FilterOp>('eq');
    const [value, setValue] = useState('');

    const needsValue = !VALUELESS_OPS.includes(op);

    const addFilter = () => {
      if (!column || (needsValue && !value.trim())) return;

      const filter: ColumnFilter = {
        column,
        op,
        // 'in' splits by comma; others use a single value (BACKEND.md §3)
        values: needsValue
          ? op === 'in'
            ? value.split(',').map(v => v.trim())
            : [value]
          : [],
      };

      onChange([...filters, filter]);
      setColumn('');
      setOp('eq');
      setValue('');
      setIsOpen(false);
    };

    const removeFilter = (index: number) => {
      onChange(filters.filter((_, i) => i !== index));
    };

    return (
      <div className="flex items-center gap-1.5 flex-wrap">
        {filters.map((filter, index) => (
          <span
            key={`${filter.column}-${filter.op}-${index}`}
            className="inline-flex items-center gap-1 px-2 py-0.5 text-xs font-mono rounded-md border border-border bg-muted"
          >
            {filter.column} {OP_LABELS[filter.op]} {filter.values.join(', ')}
            <button
              type="button"
              className="inline-flex h-3.5 w-3.5 items-center justify-center rounded-sm hover:bg-accent hover:text-accent-foreground cursor-pointer"
              onClick={() => removeFilter(index)}
            >
              <HugeiconsIcon icon={Cancel01Icon} className="h-2.5 w-2.5" />
            </button>
          </span>
        ))}

        <Popover open={isOpen} onOpenChange={setIsOpen}>
          <PopoverTrigger asChild>
            <Button
              variant="outline"
              size="sm"
              className="h-6 gap-1 text-xs"
              disabled={columns.length === 0}
            >
              <HugeiconsIcon icon={FilterIcon} className="h-3 w-3" />
              Filtro
            </Button>
          </PopoverTrigger>
          <PopoverContent
            className="w-72 p-3 flex flex-col gap-2"
            align="start"
            onInteractOutside={event => {
              // A nested Select renders in its own portal — clicking it (or
              // dismissing it) must not close this popover.
              const target = event.target as Element | null;
              if (target?.closest('[data-slot="select-content"]')) {
                event.preventDefault();
              }
            }}
          >
            <Select value={column} onValueChange={setColumn}>
              <SelectTrigger className="h-8 text-xs">
                <SelectValue placeholder="Coluna" />
              </SelectTrigger>
              <SelectContent>
                {columns.map(col => (
                  <SelectItem key={col.name} value={col.name}>
                    {col.name}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>

            <Select value={op} onValueChange={v => setOp(v as FilterOp)}>
              <SelectTrigger className="h-8 text-xs">
                <SelectValue placeholder="Operador" />
              </SelectTrigger>
              <SelectContent>
                {(Object.keys(OP_LABELS) as FilterOp[]).map(filterOp => (
                  <SelectItem key={filterOp} value={filterOp}>
                    {OP_LABELS[filterOp]}
                  </SelectItem>
                ))}
              </SelectContent>
            </Select>

            {needsValue && (
              <Input
                size="sm"
                placeholder={
                  op === 'in' ? 'valores separados por vírgula' : 'valor'
                }
                value={value}
                onChange={e => setValue(e.target.value)}
                onKeyDown={e => e.key === 'Enter' && addFilter()}
              />
            )}

            <Button size="sm" className="gap-1" onClick={addFilter}>
              <HugeiconsIcon icon={Add01Icon} className="h-3 w-3" />
              Adicionar
            </Button>
          </PopoverContent>
        </Popover>
      </div>
    );
  },
);

FilterBar.displayName = 'FilterBar';
