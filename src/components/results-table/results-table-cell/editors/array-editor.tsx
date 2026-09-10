import { memo, useEffect, useState } from 'react';

import {
  formatPgArray,
  parsePgArray,
  pgArrayTypeLabel,
  type PgArrayElement,
} from '@/lib/pg-array';
import { cn } from '@/lib/utils';

import { EditorFooter } from './editor-footer';

interface ArrayEditorProps {
  value: string;
  /** Nome cru do tipo (`_varchar`, `text[]`) — só para o rótulo. */
  columnType: string;
  onSave: (value: string) => void;
  onCancel: () => void;
  onSetNull?: () => void;
}

type Mode = 'list' | 'raw';

/** Literal cru aceito: `{...}` ou o prefixo de dimensões `[1:2]={...}`. */
const RAW_LITERAL = /^(\[[\d:,\s]+\]=)?\{[\s\S]*\}$/;

export const ArrayEditor = memo(function ArrayEditor({
  value,
  columnType,
  onSave,
  onCancel,
  onSetNull,
}: ArrayEditorProps) {
  const [items, setItems] = useState<PgArrayElement[]>([]);
  const [raw, setRaw] = useState(value);
  // Multidimensional ou literal fora do padrão só dá para editar como texto.
  const [mode, setMode] = useState<Mode>('list');
  const [error, setError] = useState<string | null>(null);
  const [focusIndex, setFocusIndex] = useState<number | null>(null);

  useEffect(() => {
    // Célula NULL/vazia (inclusive linha nova) começa como lista vazia.
    const parsed = value.trim() === '' ? [] : parsePgArray(value);
    setRaw(value);
    setItems(parsed ?? []);
    setMode(parsed === null ? 'raw' : 'list');
    setError(null);
    setFocusIndex(null);
  }, [value]);

  const preview = formatPgArray(items);

  const handleSave = () => {
    if (mode === 'list') {
      onSave(preview);
      return;
    }

    const trimmed = raw.trim();
    if (!RAW_LITERAL.test(trimmed)) {
      setError('Literal de array inválido — use o formato {a,b}');
      return;
    }
    onSave(trimmed);
  };

  const toggleMode = () => {
    setError(null);
    if (mode === 'list') {
      setRaw(preview);
      setMode('raw');
      return;
    }

    const parsed = parsePgArray(raw);
    if (parsed === null) {
      setError(
        'Não dá para editar em lista (array multidimensional ou literal inválido)',
      );
      return;
    }
    setItems(parsed);
    setMode('list');
  };

  const updateItem = (index: number, next: PgArrayElement) => {
    setItems(current => current.map((item, i) => (i === index ? next : item)));
  };

  const insertItem = (index: number) => {
    setItems(current => [
      ...current.slice(0, index),
      '',
      ...current.slice(index),
    ]);
    setFocusIndex(index);
  };

  const removeItem = (index: number) => {
    setItems(current => current.filter((_, i) => i !== index));
    setFocusIndex(null);
  };

  const handleKeyDown = (e: React.KeyboardEvent) => {
    if (e.key === 'Escape') onCancel();
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      handleSave();
    }
  };

  return (
    <div className="flex flex-col gap-2" onKeyDown={handleKeyDown}>
      <div className="flex items-center justify-between">
        <label className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider">
          Editar {pgArrayTypeLabel(columnType)}
        </label>
        <button
          type="button"
          className="px-1.5 py-0.5 text-[10px] rounded border border-border hover:bg-muted/60 transition-colors"
          onClick={toggleMode}
        >
          {mode === 'list' ? 'Editar literal' : 'Editar em lista'}
        </button>
      </div>

      {mode === 'list' ? (
        <div className="flex flex-col gap-1">
          {items.length === 0 && (
            <span className="text-[10px] text-muted-foreground italic py-1">
              Array vazio
            </span>
          )}

          <div className="flex flex-col gap-1 max-h-[200px] overflow-y-auto">
            {items.map((item, index) => (
              <div key={index} className="flex items-center gap-1">
                <span className="w-5 shrink-0 text-right text-[10px] text-muted-foreground font-mono">
                  {index + 1}
                </span>
                <input
                  type="text"
                  ref={el => {
                    if (el && focusIndex === index) {
                      el.focus();
                      setFocusIndex(null);
                    }
                  }}
                  className={cn(
                    'flex-1 min-w-0 px-2 py-1 text-xs font-mono bg-background',
                    'border border-border rounded-md',
                    'focus:outline-none focus:ring-1 focus:ring-ring',
                    item === null && 'text-muted-foreground italic',
                  )}
                  value={item ?? 'NULL'}
                  readOnly={item === null}
                  onChange={e => updateItem(index, e.target.value)}
                  onKeyDown={e => {
                    if (e.key === 'Enter' && !e.ctrlKey && !e.metaKey) {
                      e.preventDefault();
                      insertItem(index + 1);
                    }
                  }}
                  autoFocus={index === 0}
                  spellCheck={false}
                />
                <button
                  type="button"
                  title={item === null ? 'Voltar para texto' : 'Definir NULL'}
                  className={cn(
                    'shrink-0 h-6 w-6 inline-flex items-center justify-center rounded-sm',
                    'text-xs text-muted-foreground hover:bg-muted transition-colors',
                    item === null && 'text-foreground bg-muted',
                  )}
                  onClick={() => updateItem(index, item === null ? '' : null)}
                >
                  ∅
                </button>
                <button
                  type="button"
                  title="Remover item"
                  className="shrink-0 h-6 w-6 inline-flex items-center justify-center rounded-sm text-xs text-muted-foreground hover:bg-muted transition-colors"
                  onClick={() => removeItem(index)}
                >
                  ×
                </button>
              </div>
            ))}
          </div>

          <button
            type="button"
            className="self-start px-1.5 py-0.5 text-[10px] rounded border border-border hover:bg-muted/60 transition-colors"
            onClick={() => insertItem(items.length)}
          >
            + Adicionar item
          </button>

          <span
            className="text-[10px] text-muted-foreground font-mono truncate"
            title={preview}
          >
            {preview}
          </span>
        </div>
      ) : (
        <textarea
          className={cn(
            'w-full min-h-[120px] max-h-[240px] p-2 text-xs font-mono',
            'bg-background border rounded-md resize-y',
            'focus:outline-none focus:ring-1 focus:ring-ring',
            error ? 'border-red-500' : 'border-border',
          )}
          value={raw}
          onChange={e => {
            setRaw(e.target.value);
            if (error) setError(null);
          }}
          spellCheck={false}
          autoFocus
        />
      )}

      {error && <span className="text-[10px] text-red-400">{error}</span>}

      <EditorFooter
        onSave={handleSave}
        onCancel={onCancel}
        onSetNull={onSetNull}
      />
    </div>
  );
});
