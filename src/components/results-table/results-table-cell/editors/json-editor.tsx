import { memo, useEffect, useState } from 'react';
import { EditorFooter } from './editor-footer';

interface JsonEditorProps {
  value: string;
  onSave: (value: string) => void;
  onCancel: () => void;
}

export const JsonEditor = memo(function JsonEditor({
  value,
  onSave,
  onCancel,
}: JsonEditorProps) {
  const [editValue, setEditValue] = useState(value);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setEditValue(value);
  }, [value]);

  const validate = (v: string): boolean => {
    try {
      JSON.parse(v);
      setError(null);
      return true;
    } catch {
      setError('JSON inválido');
      return false;
    }
  };

  const handleFormat = () => {
    try {
      const parsed = JSON.parse(editValue);
      setEditValue(JSON.stringify(parsed, null, 2));
      setError(null);
    } catch {
      setError('JSON inválido — não foi possível formatar');
    }
  };

  const handleMinify = () => {
    try {
      const parsed = JSON.parse(editValue);
      setEditValue(JSON.stringify(parsed));
      setError(null);
    } catch {
      setError('JSON inválido — não foi possível minificar');
    }
  };

  const handleSave = () => {
    if (validate(editValue)) onSave(editValue);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Escape') onCancel();
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      handleSave();
    }
    if (e.key === 'Tab') {
      e.preventDefault();
      const target = e.currentTarget;
      const start = target.selectionStart;
      const end = target.selectionEnd;
      const newValue =
        editValue.substring(0, start) + '  ' + editValue.substring(end);
      setEditValue(newValue);
      requestAnimationFrame(() => {
        target.selectionStart = target.selectionEnd = start + 2;
      });
    }
  };

  return (
    <div className="flex flex-col gap-2">
      <div className="flex items-center justify-between">
        <label className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider">
          Editar JSON
        </label>
        <div className="flex gap-1">
          <button
            type="button"
            className="px-1.5 py-0.5 text-[10px] rounded border border-border hover:bg-muted/60 transition-colors"
            onClick={handleFormat}
          >
            Formatar
          </button>
          <button
            type="button"
            className="px-1.5 py-0.5 text-[10px] rounded border border-border hover:bg-muted/60 transition-colors"
            onClick={handleMinify}
          >
            Minificar
          </button>
        </div>
      </div>
      <textarea
        className={
          'w-full min-h-[120px] max-h-[240px] p-2 text-xs font-mono ' +
          'bg-background border rounded-md resize-y ' +
          'focus:outline-none focus:ring-1 focus:ring-ring ' +
          (error ? 'border-red-500' : 'border-border')
        }
        value={editValue}
        onChange={e => {
          setEditValue(e.target.value);
          if (error) validate(e.target.value);
        }}
        onKeyDown={handleKeyDown}
        spellCheck={false}
        autoFocus
      />
      {error && <span className="text-[10px] text-red-400">{error}</span>}
      <EditorFooter onSave={handleSave} onCancel={onCancel} />
    </div>
  );
});
