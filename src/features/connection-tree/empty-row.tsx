interface EmptyRowProps {
  level: number;
  message: string;
}

/** Pasta aberta sem nada dentro: a linha diz isso em vez de a pasta sumir. */
export const EmptyRow = ({ level, message }: EmptyRowProps) => (
  <div
    className="flex h-7 w-full items-center gap-1.5 pr-2 text-small text-fg-muted"
    style={{ paddingLeft: `${6 + level * 14}px` }}
  >
    <span className="size-3.5 shrink-0" />
    <span className="truncate">{message}</span>
  </div>
);
