interface ErrorRowProps {
  level: number;
  message: string;
  onRetry: () => void;
}

export const ErrorRow = ({ level, message, onRetry }: ErrorRowProps) => {
  return (
    <div
      className="flex items-center gap-2 py-1 px-2 text-xs text-destructive"
      style={{ paddingLeft: `${level * 1.25 + 0.5}rem` }}
    >
      <span className="truncate" title={message}>
        Falha ao carregar
      </span>
      <button
        type="button"
        className="underline hover:no-underline shrink-0"
        onClick={onRetry}
      >
        tentar novamente
      </button>
    </div>
  );
};
