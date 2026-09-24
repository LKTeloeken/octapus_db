import { HugeiconsIcon } from '@hugeicons/react';
import { Cancel01Icon } from '@hugeicons/core-free-icons';
import { memo } from 'react';
import { cn } from '@/lib/utils';
import type { ResultsTableValuePanelProps } from './results-table-value-panel.types';
import { useResultsTableValuePanel } from './use-results-table-value-panel';
import { ValueEditor } from './value-editor';
import { ValueFormatMenu } from './value-format-menu';

const HEADER_BUTTON_CLASS =
  'inline-flex h-6 items-center rounded border border-border px-1.5 text-[11px] text-muted-foreground hover:bg-muted/60 hover:text-foreground transition-colors';

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
      <div className="flex h-full min-w-0 flex-col bg-sidebar">
        <div className="flex h-9 shrink-0 items-center gap-1.5 border-b border-border px-2">
          <div className="flex min-w-0 flex-1 items-baseline gap-1.5">
            {target ? (
              <>
                <span
                  className="truncate font-mono text-xs font-medium"
                  title={target.column.name}
                >
                  {target.column.name}
                </span>
                <span className="truncate text-[10px] text-muted-foreground">
                  {target.column.typeName}
                </span>
              </>
            ) : (
              <span className="text-xs font-medium text-muted-foreground">
                Valor
              </span>
            )}
          </div>

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
            <button
              type="button"
              className={HEADER_BUTTON_CLASS}
              title="Desfazer a edição pendente desta célula"
              onClick={revert}
            >
              Desfazer
            </button>
          )}

          {target?.isEditable && target.value !== null && (
            <button
              type="button"
              className={cn(HEADER_BUTTON_CLASS, 'italic')}
              title="Definir NULL"
              onClick={setNull}
            >
              NULL
            </button>
          )}

          {onClose && (
            <button
              type="button"
              className="inline-flex h-6 w-6 items-center justify-center rounded text-muted-foreground hover:bg-muted/60 hover:text-foreground transition-colors"
              title="Fechar painel de valor"
              onClick={onClose}
            >
              <HugeiconsIcon icon={Cancel01Icon} className="size-3.5" />
            </button>
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
            <div className="flex h-full items-center justify-center p-4 text-center text-xs text-muted-foreground">
              Selecione uma célula para ver o valor
            </div>
          )}
        </div>

        {target && (
          <div className="flex shrink-0 items-center gap-2 border-t border-border px-2 py-1 text-[10px] text-muted-foreground">
            {issue ? (
              <span
                className={cn(
                  'truncate',
                  issue.blocking ? 'text-red-400' : 'text-yellow-400',
                )}
              >
                {issue.message}
              </span>
            ) : (
              <span>Linha {target.rowIndex + 1}</span>
            )}
            <span className="ml-auto flex shrink-0 items-center gap-2">
              {target.isModified && (
                <span className="text-yellow-400">pendente</span>
              )}
              {!target.isEditable ? (
                <span>somente leitura</span>
              ) : (
                format === 'binary' && <span>visão binária: só leitura</span>
              )}
              <span className={cn(sizeLabel === null && 'italic')}>
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
