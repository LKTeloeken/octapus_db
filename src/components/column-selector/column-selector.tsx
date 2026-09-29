import {
  LayoutThreeColumnIcon,
  Search01Icon,
} from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { memo, useEffect, useMemo, useRef, useState } from 'react';
import { Button } from '@/components/ui/button';
import { Checkbox } from '@/components/ui/checkbox';
import { Input } from '@/components/ui/input/input';
import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover';
import type { ColumnSelectorProps } from './column-selector.types';

/**
 * Popup para escolher as colunas exibidas no front (não altera o query).
 * Semântica: nada marcado = todas visíveis; marcar uma coluna exibe SÓ as
 * marcadas (cada marcação soma). Desmarcar a última volta a exibir todas.
 * Cmd/Ctrl+Shift+H abre o popup já com a busca focada.
 */
export const ColumnSelector = memo(
  ({ columns, hiddenColumns, onChange }: ColumnSelectorProps) => {
    const [open, setOpen] = useState(false);
    const [search, setSearch] = useState('');
    const searchInputRef = useRef<HTMLInputElement>(null);

    useEffect(() => {
      const handler = (event: KeyboardEvent) => {
        if (
          (event.metaKey || event.ctrlKey) &&
          event.shiftKey &&
          event.key.toLowerCase() === 'h'
        ) {
          event.preventDefault();
          setOpen(true);
        }
      };

      window.addEventListener('keydown', handler);
      return () => window.removeEventListener('keydown', handler);
    }, []);

    const filtered = useMemo(() => {
      const term = search.trim().toLowerCase();
      if (!term) return columns;
      return columns.filter(col => col.name.toLowerCase().includes(term));
    }, [columns, search]);

    // Nenhuma coluna oculta = modo "todas visíveis": nada aparece marcado.
    const allVisible = hiddenColumns.size === 0;
    const visibleCount = columns.reduce(
      (acc, col) => acc + (hiddenColumns.has(col.name) ? 0 : 1),
      0,
    );

    const isChecked = (name: string) => !allVisible && !hiddenColumns.has(name);

    const toggleColumn = (name: string) => {
      // Primeira marcação a partir de "todas visíveis" → exibe só esta coluna.
      if (allVisible) {
        onChange(columns.filter(col => col.name !== name).map(col => col.name));
        return;
      }

      const next = new Set(hiddenColumns);
      if (next.has(name)) next.delete(name);
      else next.add(name);

      // Desmarcar a última coluna marcada volta ao modo "todas visíveis".
      if (next.size >= columns.length) {
        onChange([]);
        return;
      }

      onChange(Array.from(next));
    };

    return (
      <Popover open={open} onOpenChange={setOpen}>
        <PopoverTrigger asChild>
          <Button
            variant="outline"
            size="sm"
            disabled={columns.length === 0}
            title="Filtrar colunas (Cmd/Ctrl+Shift+H)"
          >
            <HugeiconsIcon icon={LayoutThreeColumnIcon} />
            Colunas
            <span className="tabular-nums text-fg-subtle">
              {visibleCount}/{columns.length}
            </span>
          </Button>
        </PopoverTrigger>
        <PopoverContent
          className="flex w-64 flex-col gap-1 p-1.5"
          align="start"
          onOpenAutoFocus={event => {
            event.preventDefault();
            searchInputRef.current?.focus();
          }}
        >
          <Input
            ref={searchInputRef}
            size="sm"
            placeholder="Buscar coluna"
            value={search}
            onChange={e => setSearch(e.target.value)}
            className="mb-1"
            InputProps={{
              startAdornment: (
                <HugeiconsIcon icon={Search01Icon} className="size-3.5" />
              ),
            }}
          />

          <Button
            variant="ghost"
            size="sm"
            className="justify-start px-2 font-normal"
            disabled={allVisible}
            onClick={() => onChange([])}
          >
            Mostrar todas
          </Button>

          <div className="flex max-h-60 flex-col overflow-y-auto scrollbar-thin">
            {filtered.length === 0 ? (
              <span className="px-2 py-1.5 text-small text-fg-subtle">
                Nenhuma coluna encontrada
              </span>
            ) : (
              filtered.map(col => (
                <label
                  key={col.name}
                  className="flex h-7 shrink-0 cursor-pointer items-center gap-2 rounded-sm px-2 text-body text-fg transition-colors hover:bg-active"
                >
                  <Checkbox
                    checked={isChecked(col.name)}
                    onCheckedChange={() => toggleColumn(col.name)}
                  />
                  <span className="truncate" title={col.name}>
                    {col.name}
                  </span>
                </label>
              ))
            )}
          </div>
        </PopoverContent>
      </Popover>
    );
  },
);

ColumnSelector.displayName = 'ColumnSelector';
