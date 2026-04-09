import { memo, useEffect, useState } from 'react';
import { EditorFooter } from './editor-footer';

interface TextEditorProps {
  value: string;
  onSave: (value: string) => void;
  onCancel: () => void;
}

export const TextEditor = memo(function TextEditor({
  value,
  onSave,
  onCancel,
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
      <label className="text-[10px] font-medium text-muted-foreground uppercase tracking-wider">
        Editar texto
      </label>
      <textarea
        className="w-full min-h-[80px] max-h-[200px] p-2 text-xs font-mono bg-background border border-border rounded-md resize-y focus:outline-none focus:ring-1 focus:ring-ring"
        value={editValue}
        onChange={e => setEditValue(e.target.value)}
        onKeyDown={handleKeyDown}
        autoFocus
      />
      <EditorFooter onSave={() => onSave(editValue)} onCancel={onCancel} />
    </div>
  );
});
