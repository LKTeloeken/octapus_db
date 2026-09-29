import { cn } from '@/lib/utils';

interface LoadingStateProps {
  className?: string;
}

// Larguras fixas (fração da coluna) para o skeleton não "pular" entre renders.
const SKELETON_ROWS = [0.9, 0.7, 0.8, 0.6, 0.75, 0.5, 0.85, 0.65, 0.7, 0.55];

/**
 * Carregando (DESIGN.md §6): barra indeterminada de 2 px no topo + skeleton de
 * linhas com brilho. Com movimento reduzido o brilho para; a barra continua,
 * porque é informação.
 */
export const LoadingState = ({ className }: LoadingStateProps) => {
  return (
    <div
      role="status"
      aria-label="Executando consulta"
      className={cn('relative flex h-full flex-col overflow-hidden', className)}
    >
      <div className="relative h-0.5 shrink-0 overflow-hidden bg-hover">
        <span className="absolute inset-y-0 left-0 w-[30%] rounded-full bg-iris-text animate-indeterminate" />
      </div>
      <div className="h-10 shrink-0 border-b border-line bg-surface-2" />
      {SKELETON_ROWS.map((width, index) => (
        <div
          key={index}
          className="flex h-7 shrink-0 items-center gap-4 border-b border-line-subtle px-3"
        >
          <span className="h-2 w-6 rounded-xs bg-hover" />
          <span
            className="h-2 rounded-xs bg-[linear-gradient(90deg,var(--hover)_0%,var(--active)_50%,var(--hover)_100%)] bg-size-[300px_100%] animate-shimmer motion-reduce:animate-none"
            style={{ width: `${Math.round(width * 220)}px` }}
          />
          <span className="ml-auto h-2 w-16 rounded-xs bg-hover" />
        </div>
      ))}
    </div>
  );
};
