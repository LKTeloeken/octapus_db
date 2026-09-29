import { memo } from 'react';

import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover';
import { Checkbox } from '@/components/ui/checkbox';
import { cn } from '@/lib/utils';

import type { DataTableCellProps } from './results-table-cell.types';
import { CELL_TYPE_COLOR } from './cell-type-style';
import {
  isBooleanTrue,
  nextBooleanValue,
  resolveCellEditor,
} from './use-resolve-cell-editor';
import { TextEditor } from './editors/text-editor';
import { NumberEditor } from './editors/number-editor';
import { JsonEditor } from './editors/json-editor';
import { ArrayEditor } from './editors/array-editor';
import { DateEditor } from './editors/date-editor';
import { UuidEditor } from './editors/uuid-editor';

export const DataTableCell = memo(
  ({
    value,
    displayValue,
    isEditable,
    isModified,
    isRemoved = false,
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
            'group flex h-full w-full items-center gap-2 px-2.5',
            isModified
              ? 'bg-warning-soft'
              : 'transition-[background-color] duration-600 ease-standard motion-reduce:duration-120',
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
              'font-mono text-small',
              isNull ? 'text-fg-subtle italic' : 'text-data-bool',
              isRemoved && 'text-fg-subtle line-through',
            )}
          >
            {isNull ? 'NULL' : isTrue ? 'true' : 'false'}
          </span>
          {isEditable && !isNull && (
            <button
              type="button"
              title="Definir NULL"
              className="ml-auto hidden size-4 items-center justify-center rounded-xs text-xs text-fg-subtle group-hover:inline-flex hover:bg-hover hover:text-fg"
              onClick={() => updateCell(rowIndex, columnName, value, null)}
            >
              ∅
            </button>
          )}
        </div>
      );
    }

    // Cor pelo tipo; números à direita e tabulares; NULL em itálico apagado.
    const triggerClassName = cn(
      'block h-full w-full truncate px-2.5 text-left font-mono text-small leading-[27px]',
      'cursor-pointer outline-none',
      // A tinta aparece na hora e some devagar (600 ms) depois do salvar: o
      // CSS usa a transição do estado de destino, então ela só vale sem tinta.
      !isModified &&
        'transition-[background-color] duration-600 ease-standard motion-reduce:duration-120',
      editorType === 'number' && 'text-right tabular-nums',
      isNull ? 'text-fg-subtle italic' : CELL_TYPE_COLOR[editorType],
      isModified && 'bg-warning-soft',
      isRemoved && 'text-fg-subtle line-through',
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
          onFocus={() => console.log('focus')}
        >
          {text}
        </button>
      );
    }

    const renderEditor = () => {
      if (!isEditable) {
        return (
          <div className="max-h-[240px] overflow-auto font-mono text-small break-all whitespace-pre-wrap text-fg">
            {isNull ? (
              <span className="text-fg-subtle italic">NULL</span>
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

        case 'array':
          return (
            <ArrayEditor
              value={editText}
              columnType={columnType}
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
            editorType === 'json' || editorType === 'array' ? 'w-96' : 'w-80',
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
