import { HugeiconsIcon } from '@hugeicons/react';
import {
  ArrowDown01Icon,
  ArrowUp01Icon,
  Cancel01Icon,
  FilterHorizontalIcon,
  Search01Icon,
} from '@hugeicons/core-free-icons';
import { memo } from 'react';
import { Button } from '@/components/ui/button';
import { Checkbox } from '@/components/ui/checkbox';
import { Input } from '@/components/ui/input/input';
import { SegmentedControl } from '@/components/ui/segmented-control/segmented-control';
import type { SegmentedControlOption } from '@/components/ui/segmented-control/segmented-control.types';
import type {
  GridSearchColumnScope,
  GridSearchMode,
} from '@/lib/grid-search';
import { shortcut } from '@/lib/platform';
import type { ResultsTableSearchProps } from './results-table-search.types';
import { useResultsTableSearch } from './use-results-table-search';

const MODE_OPTIONS: SegmentedControlOption<GridSearchMode>[] = [
  { value: 'contains', label: 'Contém' },
  { value: 'word', label: 'Palavra' },
  { value: 'exact', label: 'Exato' },
  { value: 'prefix', label: 'Início' },
  { value: 'regex', label: 'Regex' },
];

const SCOPE_OPTIONS: SegmentedControlOption<GridSearchColumnScope>[] = [
  { value: 'all', label: 'Todas' },
  { value: 'only', label: 'Somente' },
  { value: 'except', label: 'Exceto' },
];

const MODE_HINT: Record<GridSearchMode, string> = {
  contains: 'O texto aparece em qualquer parte do valor',
  word: 'O texto aparece como palavra inteira',
  exact: 'O valor inteiro é igual ao texto',
  prefix: 'O valor começa com o texto',
  regex: 'Expressão regular (sintaxe do JavaScript)',
};

/**
 * Painel flutuante da busca na grade (Cmd/Ctrl+F): vidro no canto superior
 * direito, sem overlay — os destaques continuam visíveis por baixo.
 */
export const ResultsTableSearch = memo((props: ResultsTableSearchProps) => {
  const { search } = props;
  const {
    query,
    setQuery,
    options,
    setOptions,
    close,
    inputRef,
    error,
    matchCount,
    next,
    prev,
  } = search;
  const {
    status,
    showOptions,
    toggleOptions,
    activeOptionsCount,
    columnFilter,
    setColumnFilter,
    filteredColumns,
    isColumnListed,
    toggleColumn,
    onInputKeyDown,
    onPanelKeyDown,
  } = useResultsTableSearch(props);

  return (
    <div
      data-grid-search
      role="search"
      onKeyDown={onPanelKeyDown}
      className="glass absolute top-2 right-3 z-40 flex w-[min(400px,calc(100%-24px))] flex-col gap-2 rounded-lg p-1.5 animate-in fade-in-0 slide-in-from-top-2 duration-180 ease-out"
    >
      <div className="flex items-center gap-1">
        <Input
          ref={inputRef}
          size="sm"
          placeholder="Buscar na grade"
          aria-label="Buscar na grade"
          className="min-w-0 flex-1"
          value={query}
          error={!!error}
          onChange={event => setQuery(event.target.value)}
          onKeyDown={onInputKeyDown}
          InputProps={{
            startAdornment: (
              <HugeiconsIcon icon={Search01Icon} className="size-3.5" />
            ),
            endAdornment: status && (
              <span
                aria-live="polite"
                className="text-small whitespace-nowrap tabular-nums text-fg-subtle"
              >
                {status}
              </span>
            ),
          }}
        />
        <Button
          variant="ghost"
          size="icon"
          aria-label="Resultado anterior"
          title={`Anterior (⇧↵ · ${shortcut('⇧G')})`}
          disabled={matchCount === 0}
          onClick={prev}
        >
          <HugeiconsIcon icon={ArrowUp01Icon} />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          aria-label="Próximo resultado"
          title={`Próximo (↵ · ${shortcut('G')})`}
          disabled={matchCount === 0}
          onClick={next}
        >
          <HugeiconsIcon icon={ArrowDown01Icon} />
        </Button>
        <Button
          variant="ghost"
          size="icon"
          aria-label="Opções da busca"
          aria-pressed={showOptions}
          title="Opções da busca"
          className="relative"
          onClick={toggleOptions}
        >
          <HugeiconsIcon icon={FilterHorizontalIcon} />
          {activeOptionsCount > 0 && (
            <span className="absolute -top-1 -right-1 flex h-4 min-w-4 items-center justify-center rounded-full bg-surface-3 px-0.5 text-[11px] leading-none font-semibold text-fg tabular-nums shadow-[inset_0_0_0_1px_var(--line-strong)]">
              {activeOptionsCount}
            </span>
          )}
        </Button>
        <Button
          variant="ghost"
          size="icon"
          aria-label="Fechar busca"
          title={`Fechar (esc · ${shortcut('F')})`}
          onClick={close}
        >
          <HugeiconsIcon icon={Cancel01Icon} />
        </Button>
      </div>

      {error && (
        <p className="px-1 text-small text-danger" role="alert">
          {error}
        </p>
      )}

      {showOptions && (
        <div className="flex flex-col gap-2.5 border-t border-line px-1 pt-2.5 pb-1">
          <div className="flex flex-col gap-1.5">
            <span className="text-micro font-medium uppercase text-fg-subtle">
              Casamento
            </span>
            <SegmentedControl
              aria-label="Modo de casamento"
              size="xs"
              value={options.mode}
              onValueChange={mode => setOptions({ mode })}
              options={MODE_OPTIONS}
            />
            <span className="text-small text-fg-muted">
              {MODE_HINT[options.mode]}
            </span>
          </div>

          <div className="flex flex-col gap-1">
            <label className="flex h-6 cursor-pointer items-center gap-2 text-body text-fg">
              <Checkbox
                checked={options.caseSensitive}
                onCheckedChange={checked =>
                  setOptions({ caseSensitive: checked === true })
                }
              />
              Diferenciar maiúsculas
            </label>
            <label className="flex h-6 cursor-pointer items-center gap-2 text-body text-fg">
              <Checkbox
                checked={options.ignoreAccents}
                onCheckedChange={checked =>
                  setOptions({ ignoreAccents: checked === true })
                }
              />
              Ignorar acentos
            </label>
          </div>

          <div className="flex flex-col gap-1.5">
            <span className="text-micro font-medium uppercase text-fg-subtle">
              Colunas
            </span>
            <SegmentedControl
              aria-label="Colunas da busca"
              size="xs"
              value={options.columnScope}
              onValueChange={columnScope => setOptions({ columnScope })}
              options={SCOPE_OPTIONS}
            />

            {options.columnScope !== 'all' && (
              <>
                <Input
                  size="sm"
                  placeholder="Filtrar colunas"
                  aria-label="Filtrar colunas"
                  value={columnFilter}
                  onChange={event => setColumnFilter(event.target.value)}
                  InputProps={{
                    startAdornment: (
                      <HugeiconsIcon icon={Search01Icon} className="size-3.5" />
                    ),
                  }}
                />
                <div className="flex max-h-44 flex-col overflow-y-auto scrollbar-thin">
                  {filteredColumns.length === 0 ? (
                    <span className="px-2 py-1.5 text-small text-fg-subtle">
                      Nenhuma coluna encontrada
                    </span>
                  ) : (
                    filteredColumns.map(column => (
                      <label
                        key={column.name}
                        className="flex h-7 shrink-0 cursor-pointer items-center gap-2 rounded-sm px-2 text-body text-fg transition-colors hover:bg-active"
                      >
                        <Checkbox
                          checked={isColumnListed(column.name)}
                          onCheckedChange={() => toggleColumn(column.name)}
                        />
                        <span className="truncate">{column.name}</span>
                        <span className="ml-auto truncate font-mono text-[11px] text-fg-subtle">
                          {column.typeName}
                        </span>
                      </label>
                    ))
                  )}
                </div>
              </>
            )}
          </div>
        </div>
      )}
    </div>
  );
});

ResultsTableSearch.displayName = 'ResultsTableSearch';
