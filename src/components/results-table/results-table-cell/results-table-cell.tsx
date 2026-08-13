import { memo } from 'react';

import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover';
import { Checkbox } from '@/components/ui/checkbox';
import { cn } from '@/lib/utils';

import type { DataTableCellProps } from './results-table-cell.types';
import {
  isBooleanTrue,
  nextBooleanValue,
  resolveCellEditor,
} from './use-resolve-cell-editor';
import { TextEditor } from './editors/text-editor';
import { NumberEditor } from './editors/number-editor';
import { JsonEditor } from './editors/json-editor';
import { DateEditor } from './editors/date-editor';
import { UuidEditor } from './editors/uuid-editor';

export const DataTableCell = memo(
  ({
    value,
    displayValue,
    isEditable,
    isModified,
    columnType,
    rowIndex,
    columnName,
    isActive,
    onActivate,
    onClose,
    updateCell,
  }: DataTableCellProps) => {
    const editorType = resolveCellEditor(columnType);

    // Olha o valor exibido (inclui mudança pendente), não só o original —
    // assim um NULL pendente aparece como NULL.
    const isNull = displayValue === null;
    const text = displayValue ?? 'NULL';
    const editText = displayValue ?? '';

    const handleSave = (newValue: string | null) => {
      updateCell(rowIndex, columnName, value, newValue);
      onClose();
    };

    // Booleans sempre renderizam checkbox + true/false normalizado (o Postgres
    // devolve "t"/"f" cru); edição é inline, sem popover.
    if (editorType === 'boolean' && (isEditable || !isNull)) {
      const isTrue = !isNull && isBooleanTrue(text);

      return (
        <div
          className={cn(
            'group flex items-center gap-2 w-full px-2 py-1.5',
            isModified && 'bg-yellow-900/30',
          )}
        >
          <Checkbox
            checked={isNull ? 'indeterminate' : isTrue}
            disabled={!isEditable}
            onCheckedChange={() => {
              updateCell(
                rowIndex,
                columnName,
                value,
                nextBooleanValue(displayValue),
              );
            }}
            className="h-3.5 w-3.5"
          />
          <span
            className={cn(
              'text-xs font-mono',
              isNull && 'text-muted-foreground italic',
              isModified && 'text-yellow-200',
            )}
          >
            {isNull ? 'NULL' : isTrue ? 'true' : 'false'}
          </span>
          {isEditable && !isNull && (
            <button
              type="button"
              title="Definir NULL"
              className="ml-auto hidden group-hover:inline-flex h-4 w-4 items-center justify-center rounded-sm text-xs text-muted-foreground hover:bg-muted"
              onClick={() => updateCell(rowIndex, columnName, value, null)}
            >
              ∅
            </button>
          )}
        </div>
      );
    }

    const triggerClassName = cn(
      'w-full text-left font-mono text-xs truncate block px-2 py-1.5 rounded-sm',
      'hover:bg-muted/60 transition-colors cursor-pointer outline-none',
      'focus-visible:ring-1 focus-visible:ring-ring',
      isNull && 'text-muted-foreground italic',
      isModified && 'bg-yellow-900/30 text-yellow-200',
    );

    // Closed cell: a plain button, no Radix Popover mounted. Only the single
    // active cell (below) mounts a Popover, keeping scroll/interaction cheap.
    // A single click bubbles to the row (selection); a double click opens the
    // editor.
    if (!isActive) {
      return (
        <button
          type="button"
          // Fora da ordem de Tab: o único tab stop da grade é o container.
          tabIndex={-1}
          className={triggerClassName}
          onDoubleClick={() => onActivate(rowIndex, columnName)}
        >
          {text}
        </button>
      );
    }

    const renderEditor = () => {
      if (!isEditable) {
        return (
          <div className="text-xs font-mono whitespace-pre-wrap break-all max-h-[240px] overflow-auto">
            {isNull ? (
              <span className="text-muted-foreground italic">NULL</span>
            ) : (
              text
            )}
          </div>
        );
      }

      const handleSetNull = () => handleSave(null);

      switch (editorType) {
        case 'number':
          return (
            <NumberEditor
              value={editText}
              onSave={handleSave}
              onCancel={onClose}
              onSetNull={handleSetNull}
            />
          );

        case 'json':
          return (
            <JsonEditor
              value={editText}
              onSave={handleSave}
              onCancel={onClose}
              onSetNull={handleSetNull}
            />
          );

        case 'date':
        case 'datetime':
        case 'time':
          return (
            <DateEditor
              value={editText}
              type={editorType}
              onSave={handleSave}
              onCancel={onClose}
              onSetNull={handleSetNull}
            />
          );

        case 'uuid':
          return (
            <UuidEditor
              value={editText}
              onSave={handleSave}
              onCancel={onClose}
              onSetNull={handleSetNull}
            />
          );

        case 'text':
        default:
          return (
            <TextEditor
              value={editText}
              onSave={handleSave}
              onCancel={onClose}
              onSetNull={handleSetNull}
            />
          );
      }
    };

    return (
      <Popover open onOpenChange={open => !open && onClose()}>
        <PopoverTrigger asChild>
          <button type="button" className={triggerClassName}>
            {text}
          </button>
        </PopoverTrigger>

        <PopoverContent
          className={cn(
            'max-h-80 overflow-auto p-3',
            editorType === 'json' ? 'w-96' : 'w-80',
          )}
          align="start"
          side="bottom"
          onOpenAutoFocus={e => {
            if (!isEditable) e.preventDefault();
          }}
        >
          {renderEditor()}
        </PopoverContent>
      </Popover>
    );
  },
);

DataTableCell.displayName = 'DataTableCell';
