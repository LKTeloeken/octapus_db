// editors/editor-footer.tsx
import { memo } from 'react';

interface EditorFooterProps {
  onSave: () => void;
  onCancel: () => void;
  hint?: string;
}

export const EditorFooter = memo(function EditorFooter({
  onSave,
  onCancel,
  hint = 'Ctrl+Enter para salvar',
}: EditorFooterProps) {
  return (
    <div className="flex items-center justify-between">
      <span className="text-[10px] text-muted-foreground">{hint}</span>
      <div className="flex gap-1.5">
        <button
          type="button"
          className="px-2.5 py-1 text-xs rounded-md border border-border bg-muted hover:bg-muted/80 transition-colors"
          onClick={onCancel}
        >
          Cancelar
        </button>
        <button
          type="button"
          className="px-2.5 py-1 text-xs rounded-md bg-primary text-primary-foreground hover:bg-primary/90 transition-colors"
          onClick={onSave}
        >
          Salvar
        </button>
      </div>
    </div>
  );
});
