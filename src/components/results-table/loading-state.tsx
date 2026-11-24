import { cn } from '@/lib/utils';

interface LoadingStateProps {
  className?: string;
}

// Larguras fixas (fração da coluna) para o skeleton não "pular" entre renders.
const SKELETON_ROWS = [0.9, 0.7, 0.8, 0.6, 0.75, 0.5, 0.85, 0.65, 0.7, 0.55];

/**
 * Carregando (DESIGN.md, Carregando): barra indeterminada de 2 px no topo e,
 * só depois de 400 ms sem resposta, o esqueleto das linhas — pílulas paradas,
 * sem brilho correndo. Com movimento reduzido a barra continua (é informação).
 */
export const LoadingState = ({ className }: LoadingStateProps) => {
  return (
    <div
      role="status"
      aria-label="Executando consulta"
      className={cn('relative flex h-full flex-col overflow-hidden', className)}
    >
      <div className="relative h-0.5 shrink-0 overflow-hidden bg-hover">
        <span className="absolute inset-y-0 left-0 w-[30%] rounded-full bg-tinta-text animate-indeterminate" />
      </div>
      <div className="h-10 shrink-0 border-b border-line bg-surface-2" />
      <div className="animate-in fade-in-0 fill-mode-both delay-400 duration-240">
        {SKELETON_ROWS.map((width, index) => (
          <div
            key={index}
            className="flex h-(--row-h) shrink-0 items-center gap-4 border-b border-line-subtle px-3"
          >
            <span className="h-2 w-6 rounded-full bg-hover" />
            <span
              className="h-2 rounded-full bg-active"
              style={{ width: `${Math.round(width * 220)}px` }}
            />
            <span className="ml-auto h-2 w-16 rounded-full bg-hover" />
          </div>
        ))}
      </div>
    </div>
  );
};
