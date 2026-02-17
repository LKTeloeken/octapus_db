import { memo, useEffect, useState } from 'react';
import { EditorFooter } from './editor-footer';

interface UuidEditorProps {
  value: string;
  onSave: (value: string) => void;
  onCancel: () => void;
}

const UUID_REGEX =
  /^[0-9a-f]{8}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{4}-[0-9a-f]{12}$/i;

export const UuidEditor = memo(function UuidEditor({
  value,
  onSave,
  onCancel,
}: UuidEditorProps) {
  const [editValue, setEditValue] = useState(value);
  const [error, setError] = useState<string | null>(null);

  useEffect(() => {
    setEditValue(value);
  }, [value]);

  const validate = (v: string): boolean => {
    if (!UUID_REGEX.test(v.trim())) {
      setError('UUID inválido');
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
        Editar UUID
      </label>
      <input
        type="text"
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
        placeholder="xxxxxxxx-xxxx-xxxx-xxxx-xxxxxxxxxxxx"
        autoFocus
      />
      {error && <span className="text-[10px] text-red-400">{error}</span>}
      <EditorFooter onSave={handleSave} onCancel={onCancel} />
    </div>
  );
});
