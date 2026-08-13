import { memo } from 'react';
import { Checkbox } from '@/components/ui/checkbox';

interface BooleanEditorProps {
  value: string;
  onSave: (value: string) => void;
}

export const BooleanEditor = memo(function BooleanEditor({
  value,
  onSave,
}: BooleanEditorProps) {
  const isTrue = value === 'true' || value === 't' || value === '1';

  const handleToggle = (checked: boolean) => {
    onSave(checked ? 'true' : 'false');
  };

  return (
    <div className="flex items-center gap-2">
      <Checkbox
        checked={isTrue}
        onCheckedChange={handleToggle}
        id="bool-editor"
      />
      <label
        htmlFor="bool-editor"
        className="text-xs font-mono cursor-pointer select-none"
      >
        {isTrue ? 'true' : 'false'}
      </label>
    </div>
  );
});
