import { memo } from 'react';
import { Button } from '@/components/ui/button';

interface EditorFooterProps {
  onSave: () => void;
  onCancel: () => void;
  /** Grava NULL na célula (substitui o valor atual) */
  onSetNull?: () => void;
  hint?: string;
}

export const EditorFooter = memo(function EditorFooter({
  onSave,
  onCancel,
  onSetNull,
  hint = 'Ctrl+Enter para salvar',
}: EditorFooterProps) {
  return (
    <div className="flex items-center justify-between gap-2">
      <span className="text-[11px] text-fg-subtle">{hint}</span>
      <div className="flex gap-1.5">
        {onSetNull && (
          <Button
            variant="ghost"
            size="xs"
            className="italic"
            onClick={onSetNull}
          >
            NULL
          </Button>
        )}
        <Button variant="outline" size="xs" onClick={onCancel}>
          Cancelar
        </Button>
        <Button size="xs" onClick={onSave}>
          Salvar
        </Button>
      </div>
    </div>
  );
});
