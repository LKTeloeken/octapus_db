import { memo, useEffect, useState } from 'react';
import { EditorFooter } from './editor-footer';

interface TextEditorProps {
  value: string;
  onSave: (value: string) => void;
  onCancel: () => void;
  onSetNull?: () => void;
}

export const TextEditor = memo(function TextEditor({
  value,
  onSave,
  onCancel,
  onSetNull,
}: TextEditorProps) {
  const [editValue, setEditValue] = useState(value);

  useEffect(() => {
    setEditValue(value);
  }, [value]);

  const handleKeyDown = (e: React.KeyboardEvent<HTMLTextAreaElement>) => {
    if (e.key === 'Escape') onCancel();
    if (e.key === 'Enter' && (e.ctrlKey || e.metaKey)) {
      e.preventDefault();
      onSave(editValue);
    }
  };

  return (
    <div className="flex flex-col gap-2">
      <label className="text-micro font-medium uppercase text-fg-subtle">
        Editar texto
      </label>
      <textarea
        className="w-full min-h-[80px] max-h-[200px] p-2 text-small font-mono bg-field text-fg border border-line rounded-sm resize-y outline-none focus:border-iris-text focus:ring-3 focus:ring-iris-soft"
        value={editValue}
        onChange={e => setEditValue(e.target.value)}
        onKeyDown={handleKeyDown}
        autoFocus
      />
      <EditorFooter
        onSave={() => onSave(editValue)}
        onCancel={onCancel}
        onSetNull={onSetNull}
      />
    </div>
  );
});
