import { memo, useEffect, useState } from 'react';
import { EditorFooter } from './editor-footer';

interface NumberEditorProps {
  value: string;
  onSave: (value: string) => void;
  onCancel: () => void;
}

export const NumberEditor = memo(function NumberEditor({
  value,
  onSave,
  onCancel,
}: NumberEditorProps) {
  const [editValue, setEditValue] = useState(value);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setEditValue(value);
  }, [value]);

  const validate = (v: string): boolean => {
    if (v.trim() === '') {
      setError('Valor não pode ser vazio');
      return false;
    }
    if (isNaN(Number(v))) {
      setError('Valor precisa ser um número válido');
      return false;
    }
    setError(null);
    return true;
  };

  const handleChange = (e: React.ChangeEvent<HTMLInputElement>) => {
    const v = e.target.value;
    setEditValue(v);
    if (error) validate(v);
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
        Editar número
      </label>
      <input
        type="text"
        inputMode="decimal"
        className={
          'w-full p-2 text-xs font-mono bg-background border rounded-md ' +
          'focus:outline-none focus:ring-1 focus:ring-ring ' +
          (error ? 'border-red-500' : 'border-border')
        }
        value={editValue}
        onChange={handleChange}
        onKeyDown={handleKeyDown}
        autoFocus
      />
      {error && <span className="text-[10px] text-red-400">{error}</span>}
      <EditorFooter onSave={handleSave} onCancel={onCancel} />
    </div>
  );
});
