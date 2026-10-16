import { Label } from '@/components/ui/label';
import { Switch } from '@/components/ui/switch';
import { cn } from '@/lib/utils';

interface OptionRowProps {
  id: string;
  label: string;
  hint: string;
  checked: boolean;
  disabled?: boolean;
  onChange: (checked: boolean) => void;
  /** Opção destrutiva: o rótulo em `--danger` */
  danger?: boolean;
}

/** Uma opção liga/desliga com explicação, nos diálogos de backup e restauração */
export function OptionRow({
  id,
  label,
  hint,
  checked,
  disabled,
  onChange,
  danger,
}: OptionRowProps) {
  return (
    <div
      className={cn(
        'flex items-start justify-between gap-4',
        disabled && 'opacity-45',
      )}
    >
      <div className="flex flex-col gap-0.5">
        <Label htmlFor={id} className={cn(danger && 'text-danger')}>
          {label}
        </Label>
        <p className="text-small text-fg-muted">{hint}</p>
      </div>
      <Switch
        id={id}
        checked={checked}
        disabled={disabled}
        onCheckedChange={onChange}
      />
    </div>
  );
}
