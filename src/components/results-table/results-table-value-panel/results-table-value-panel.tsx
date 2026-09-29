import { HugeiconsIcon } from '@hugeicons/react';
import { Cancel01Icon, Undo02Icon } from '@hugeicons/core-free-icons';
import { memo } from 'react';
import { Badge } from '@/components/ui/badge';
import { Button } from '@/components/ui/button';
import { cn } from '@/lib/utils';
import type { ResultsTableValuePanelProps } from './results-table-value-panel.types';
import { useResultsTableValuePanel } from './use-results-table-value-panel';
import { ValueEditor } from './value-editor';
import { ValueFormatMenu } from './value-format-menu';

/**
 * Painel lateral com o valor da célula sob o cursor da grade — o "Value
 * panel" do DBeaver. Mostra e edita o valor inteiro, com formatação de JSON,
 * lista, XML/HTML e a visão binária.
 */
export const ResultsTableValuePanel = memo(
  ({ target, updateCell, onClose, onEscape }: ResultsTableValuePanelProps) => {
    const {
      isFormattable,
      isReadOnly,
      format,
      draft,
      issue,
      editorKey,
      sizeLabel,
      wordWrap,
      autoFormat,
      saveCompact,
      encoding,
      handleChange,
      changeFormat,
      changeAutoFormat,
      changeEncoding,
      setWordWrap,
      setSaveCompact,
      setNull,
      revert,
    } = useResultsTableValuePanel({ target, updateCell });

    return (
      <div className="flex h-full min-w-0 flex-col bg-surface-1">
        <div className="flex h-10 shrink-0 items-center gap-1.5 border-b border-line-subtle pr-1.5 pl-3.5">
          <span className="shrink-0 text-body font-semibold text-fg">Valor</span>
          {target && (
            <Badge mono className="min-w-0" title={target.column.name}>
              <span className="truncate">
                {target.column.name} · {target.column.typeName}
              </span>
            </Badge>
          )}
          <span className="flex-1" />

          {target && isFormattable && (
            <ValueFormatMenu
              format={format}
              wordWrap={wordWrap}
              autoFormat={autoFormat}
              saveCompact={saveCompact}
              encoding={encoding}
              onFormatChange={changeFormat}
              onWordWrapChange={setWordWrap}
              onAutoFormatChange={changeAutoFormat}
              onSaveCompactChange={setSaveCompact}
              onEncodingChange={changeEncoding}
            />
          )}

          {target?.isModified && (
            <Button
              variant="ghost"
              size="xs"
              title="Desfazer a edição pendente desta célula"
              onClick={revert}
            >
              <HugeiconsIcon icon={Undo02Icon} />
              Desfazer
            </Button>
          )}

          {target?.isEditable && target.value !== null && (
            <Button
              variant="ghost"
              size="xs"
              className="italic"
              title="Definir NULL"
              onClick={setNull}
            >
              NULL
            </Button>
          )}

          {onClose && (
            <Button
              variant="ghost"
              size="icon-xs"
              aria-label="Fechar painel de valor"
              title="Fechar painel de valor"
              onClick={onClose}
            >
              <HugeiconsIcon icon={Cancel01Icon} />
            </Button>
          )}
        </div>

        <div className="min-h-0 flex-1 overflow-hidden">
          {target ? (
            <ValueEditor
              key={editorKey}
              value={draft}
              format={format}
              wordWrap={wordWrap}
              readOnly={isReadOnly}
              isNull={target.value === null}
              onChange={handleChange}
              onEscape={onEscape}
            />
          ) : (
            <div className="flex h-full items-center justify-center p-4 text-center text-small text-fg-subtle">
              Selecione uma célula para ver o valor
            </div>
          )}
        </div>

        {target && (
          <div className="flex h-9 shrink-0 items-center gap-2 border-t border-line-subtle px-3.5 text-small text-fg-subtle">
            {issue ? (
              <span
                className={cn(
                  'truncate',
                  issue.blocking ? 'text-danger' : 'text-warning',
                )}
              >
                {issue.message}
              </span>
            ) : target.isModified ? (
              <span className="inline-flex items-center gap-2 truncate">
                <span aria-hidden className="size-1.5 shrink-0 rounded-full bg-warning" />
                Edição pendente · linha {target.rowIndex + 1}
              </span>
            ) : (
              <span>Linha {target.rowIndex + 1}</span>
            )}
            <span className="ml-auto flex shrink-0 items-center gap-2">
              {!target.isEditable ? (
                <span>somente leitura</span>
              ) : (
                format === 'binary' && <span>visão binária: só leitura</span>
              )}
              <span
                className={cn('font-mono text-[11px]', sizeLabel === null && 'italic')}
              >
                {sizeLabel ?? 'NULL'}
              </span>
            </span>
          </div>
        )}
      </div>
    );
  },
);

ResultsTableValuePanel.displayName = 'ResultsTableValuePanel';
