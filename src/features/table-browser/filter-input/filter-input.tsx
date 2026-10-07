import { memo } from 'react';
import { QueryEditor } from '@/components/query-editor/query-editor/query-editor';
import { inputControlVariants } from '@/components/ui/input/input';
import { Kbd } from '@/components/ui/kbd';
import { cn } from '@/lib/utils';
import type { FilterInputProps } from './filter-input.types';

/**
 * WHERE do Postgres em mono 12 px; Enter aplica, Esc volta ao filtro aplicado.
 *
 * É o mesmo `QueryEditor` da aba de query, em modo de uma linha, com a moldura do
 * `Input` do Ink — então herda o autocomplete com as colunas da tabela aberta. O popup
 * de sugestão vive no `body`, por isso não é cortado pela toolbar baixa.
 */
export const FilterInput = memo(
  ({
    value,
    onChange,
    onApply,
    onReset,
    completionSource,
    completionExtensions,
  }: FilterInputProps) => {
    return (
      <div
        className={cn(
          inputControlVariants({ size: 'sm' }),
          'min-w-0 flex-1 cursor-text',
        )}
      >
        <span className="flex shrink-0 items-center font-mono text-[11px] tracking-[0.04em] text-fg-subtle">
          WHERE
        </span>
        <QueryEditor
          className="h-full min-w-0 flex-1"
          height="100%"
          singleLine
          dialect="postgres"
          value={value}
          onChange={onChange}
          onRun={onApply}
          onEscape={onReset}
          runMode="all"
          fontSize={12}
          placeholderText="id = 1 AND name ILIKE '%foo%'"
          sqlCompletionSource={completionSource}
          sqlExtraExtensions={completionExtensions}
        />
        <Kbd>↵</Kbd>
      </div>
    );
  },
);

FilterInput.displayName = 'FilterInput';
