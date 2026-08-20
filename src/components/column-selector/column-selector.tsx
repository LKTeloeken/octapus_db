import { Search01Icon, ViewIcon } from '@hugeicons/core-free-icons';
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
            className="gap-1 text-xs"
            disabled={columns.length === 0}
            title="Filtrar colunas (Cmd/Ctrl+Shift+H)"
          >
            <HugeiconsIcon icon={ViewIcon} className="h-3 w-3" />
            Colunas
            <span className="text-muted-foreground">
              {visibleCount}/{columns.length}
            </span>
          </Button>
        </PopoverTrigger>
        <PopoverContent
          className="w-64 p-2 flex flex-col gap-2"
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
            InputProps={{
              startAdornment: (
                <HugeiconsIcon icon={Search01Icon} className="h-3.5 w-3.5" />
              ),
            }}
          />

          <Button
            variant="ghost"
            size="sm"
            className="h-7 justify-start text-xs"
            disabled={allVisible}
            onClick={() => onChange([])}
          >
            Mostrar todas
          </Button>

          <div className="flex flex-col gap-0.5 max-h-60 overflow-y-auto scrollbar-thin">
            {filtered.length === 0 ? (
              <span className="px-2 py-1.5 text-xs text-muted-foreground">
                Nenhuma coluna encontrada
              </span>
            ) : (
              filtered.map(col => (
                <label
                  key={col.name}
                  className="flex items-center gap-2 px-2 py-1 rounded-sm text-xs cursor-pointer hover:bg-accent"
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
