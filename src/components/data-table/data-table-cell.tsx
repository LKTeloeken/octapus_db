import { memo, useEffect, useState } from 'react';

import {
  Popover,
  PopoverContent,
  PopoverTrigger,
} from '@/components/ui/popover';
import { cn } from '@/lib/utils';

import type { DataTableCellProps } from './data-table-cell.types';

export const DataTableCell = memo(function DataTableCell({
  value,
  displayValue,
  isEditable,
  isModified,
  onSave,
}: DataTableCellProps) {
  const [isOpen, setIsOpen] = useState(false);
  const [editValue, setEditValue] = useState(displayValue ?? '');

  const isNull = value === null && displayValue === null;
  const text = displayValue ?? 'NULL';

  // Sync local edit state when display value changes externally
  useEffect(() => {
    setEditValue(displayValue ?? '');
  }, [displayValue]);

  const handleOpenChange = (open: boolean) => {
    if (!open) {
      // Reset edit value on close without saving
      setEditValue(displayValue ?? '');
    }
    setIsOpen(open);
  };

  const handleSave = () => {
    onSave(editValue);
    setIsOpen(false);
  };

  const handleCancel = () => {
    setEditValue(displayValue ?? '');
    setIsOpen(false);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Escape') {
      handleCancel();
    }
    // Ctrl/Cmd + Enter to save
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      handleSave();
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
        className="w-80 max-h-80 overflow-auto p-3"
        align="start"
        side="bottom"
        onOpenAutoFocus={e => {
          // Prevent auto-focus stealing when not editable
          if (!isEditable || isNull) e.preventDefault();
        }}
      >
        {isEditable && !isNull ? (
          <div className="flex flex-col gap-2">
            <label className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider">
              Editar valor
            </label>
            <textarea
              className="w-full min-h-[80px] max-h-[200px] p-2 text-xs font-mono bg-background border border-border rounded-md resize-y focus:outline-none focus:ring-1 focus:ring-ring"
              value={editValue}
              onChange={e => setEditValue(e.target.value)}
              onKeyDown={handleKeyDown}
              autoFocus
            />
            <div className="flex items-center justify-between">
              <span className="text-[10px] text-muted-foreground">
                Ctrl+Enter para salvar
              </span>
              <div className="flex gap-1.5">
                <button
                  type="button"
                  className="px-2.5 py-1 text-xs rounded-md border border-border bg-muted hover:bg-muted/80 transition-colors"
                  onClick={handleCancel}
                >
                  Cancelar
                </button>
                <button
                  type="button"
                  className="px-2.5 py-1 text-xs rounded-md bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
                  onClick={handleSave}
                >
                  Salvar
                </button>
              </div>
            </div>
          </div>
        ) : (
          <div className="text-xs font-mono whitespace-pre-wrap break-all max-h-[240px] overflow-auto">
            {isNull ? (
              <span className="text-muted-foreground italic">NULL</span>
            ) : (
              text
            )}
          </div>
        )}
      </PopoverContent>
    </Popover>
  );
});
