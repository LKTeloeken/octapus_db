import { memo, useEffect, useState } from 'react';
import { EditorFooter } from './editor-footer';

interface DateEditorProps {
  value: string;
  type: 'date' | 'datetime' | 'time';
  onSave: (value: string) => void;
  onCancel: () => void;
}

export const DateEditor = memo(function DateEditor({
  value,
  type,
  onSave,
  onCancel,
}: DateEditorProps) {
  const [editValue, setEditValue] = useState(value);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setEditValue(value);
  }, [value]);

  const inputType =
    type === 'date' ? 'date' : type === 'time' ? 'time' : 'datetime-local';

  const validate = (v: string): boolean => {
    if (!v.trim()) {
      setError('Valor não pode ser vazio');
      return false;
    }
    const d = new Date(v);
    if (type !== 'time' && isNaN(d.getTime())) {
      setError('Data/hora inválida');
      return false;
    }
    setError(null);
    return true;
  };

  const handleSave = () => {
    if (validate(editValue)) onSave(editValue);
  };

  const handleKeyDown = (e: React.KeyboardEvent<HTMLInputElement>) => {
    if (e.key === 'Escape') onCancel();
    if (e.key === 'Enter') {
      e.preventDefault();
      handleSave();
    }
  };

  return (
    <div className="flex flex-col gap-2">
      <label className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider">
        Editar{' '}
        {type === 'date' ? 'data' : type === 'time' ? 'hora' : 'data/hora'}
      </label>
      <input
        type={inputType}
        step={type === 'time' || type === 'datetime' ? '1' : undefined}
        className={
          'w-full p-2 text-xs font-mono bg-background border rounded-md ' +
          'focus:outline-none focus:ring-1 focus:ring-ring ' +
          (error ? 'border-red-500' : 'border-border')
        }
        value={editValue}
        onChange={e => {
          setEditValue(e.target.value);
          if (error) validate(e.target.value);
        }}
        onKeyDown={handleKeyDown}
        autoFocus
      />
      {error && <span className="text-[10px] text-red-400">{error}</span>}
      <EditorFooter onSave={handleSave} onCancel={onCancel} />
    </div>
  );
});
