import { memo, useState } from 'react';

import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover';
import { Checkbox } from '@/components/ui/checkbox';
import { cn } from '@/lib/utils';

import type { DataTableCellProps } from './results-table-cell.types';
import { resolveCellEditor } from './use-resolve-cell-editor';
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
    onSave,
  }: DataTableCellProps) => {
    const [isOpen, setIsOpen] = useState(false);
    const editorType = resolveCellEditor(columnType);

    const isNull = value === null && displayValue === null;
    const text = displayValue ?? 'NULL';
    const editText = displayValue ?? '';

    const handleOpenChange = (open: boolean) => {
      setIsOpen(open);
    };

    const handleSave = (newValue: string) => {
      onSave(newValue);
      setIsOpen(false);
    };

    const handleCancel = () => {
      setIsOpen(false);
    };

    if (editorType === 'boolean' && isEditable && !isNull) {
      const isTrue = text === 'true' || text === 't' || text === '1';

      return (
        <div
          className={cn(
            'flex items-center gap-2 w-full px-2 py-1.5',
            isModified && 'bg-yellow-900/30',
          )}
        >
          <Checkbox
            checked={isTrue}
            onCheckedChange={checked => {
              onSave(checked ? 'true' : 'false');
            }}
            className="h-3.5 w-3.5"
          />
          <span
            className={cn('text-xs font-mono', isModified && 'text-yellow-200')}
          >
            {isTrue ? 'true' : 'false'}
          </span>
        </div>
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

      switch (editorType) {
        case 'number':
          return (
            <NumberEditor
              value={editText}
              onSave={handleSave}
              onCancel={handleCancel}
            />
          );

        case 'json':
          return (
            <JsonEditor
              value={editText}
              onSave={handleSave}
              onCancel={handleCancel}
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
              onCancel={handleCancel}
            />
          );

        case 'uuid':
          return (
            <UuidEditor
              value={editText}
              onSave={handleSave}
              onCancel={handleCancel}
            />
          );

        case 'text':
        default:
          return (
            <TextEditor
              value={editText}
              onSave={handleSave}
              onCancel={handleCancel}
            />
          );
      }
    };

    return (
      <Popover open={isOpen} onOpenChange={handleOpenChange}>
        <PopoverTrigger asChild>
          <button
            type="button"
            className={cn(
              'w-full text-left font-mono text-xs truncate block px-2 py-1.5 rounded-sm',
              'hover:bg-muted/60 transition-colors cursor-pointer outline-none',
              'focus-visible:ring-1 focus-visible:ring-ring',
              isNull && 'text-muted-foreground italic',
              isModified && 'bg-yellow-900/30 text-yellow-200',
            )}
          >
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
            if (!isEditable || isNull) e.preventDefault();
          }}
        >
          {renderEditor()}
        </PopoverContent>
      </Popover>
    );
  },
);
