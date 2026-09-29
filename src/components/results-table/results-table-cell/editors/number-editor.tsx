import { memo, useEffect, useState } from 'react';
import { EditorFooter } from './editor-footer';

interface NumberEditorProps {
  value: string;
  onSave: (value: string) => void;
  onCancel: () => void;
  onSetNull?: () => void;
}

export const NumberEditor = memo(function NumberEditor({
  value,
  onSave,
  onCancel,
  onSetNull,
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
      <label className="text-micro font-medium uppercase text-fg-subtle">
        Editar número
      </label>
      <input
        type="text"
        inputMode="decimal"
        className={
          'w-full p-2 text-small font-mono text-fg bg-field border rounded-sm ' +
          'outline-none transition-[border-color,box-shadow] focus:border-iris-text focus:ring-3 focus:ring-iris-soft ' +
          (error ? 'border-danger' : 'border-line')
        }
        value={editValue}
        onChange={handleChange}
        onKeyDown={handleKeyDown}
        autoFocus
      />
      {error && <span className="text-small text-danger">{error}</span>}
      <EditorFooter onSave={handleSave} onCancel={onCancel} onSetNull={onSetNull} />
    </div>
  );
});
