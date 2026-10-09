import type { ReactNode } from 'react';

interface SettingRowProps {
  /** id do controle — o rótulo aponta para ele */
  htmlFor?: string;
  label: string;
  description?: ReactNode;
  /** O controle (Switch, SegmentedControl…), alinhado à direita */
  children: ReactNode;
}

/** Uma configuração: rótulo e ajuda à esquerda, o controle à direita. */
export function SettingRow({
  htmlFor,
  label,
  description,
  children,
}: SettingRowProps) {
  return (
    <div className="flex items-start justify-between gap-6 border-b border-line-subtle py-3.5 first:pt-0 last:border-b-0">
      <div className="flex min-w-0 flex-col gap-1">
        {htmlFor ? (
          <label htmlFor={htmlFor} className="text-body font-medium text-fg">
            {label}
          </label>
        ) : (
          <span className="text-body font-medium text-fg">{label}</span>
        )}
        {description && (
          <p className="text-small text-fg-muted">{description}</p>
        )}
      </div>
      <div className="flex shrink-0 items-center pt-0.5">{children}</div>
    </div>
  );
}
