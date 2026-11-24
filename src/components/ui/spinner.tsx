import { cn } from '@/lib/utils';

/** Posição de cada ventosa no arco (margem de baixo, em % do tamanho) e atraso */
const SUCKERS = [
  { lift: '0%', delay: '0ms' },
  { lift: '22%', delay: '120ms' },
  { lift: '30%', delay: '240ms' },
  { lift: '22%', delay: '360ms' },
  { lift: '0%', delay: '480ms' },
] as const;

/**
 * Carregando em linha (DESIGN.md, Carregando): cinco ventosas num arco que
 * acendem em sequência, no lugar do spinner que gira. A cor vem do `text-*`
 * (`currentColor`) e o tamanho do `size-*`, como um ícone.
 */
function Spinner({ className, ...props }: React.ComponentProps<'span'>) {
  return (
    <span
      role="status"
      aria-label="Carregando"
      className={cn(
        'inline-flex size-4 shrink-0 items-end justify-center gap-[6%] pb-[18%]',
        className,
      )}
      {...props}
    >
      {SUCKERS.map(({ lift, delay }) => (
        <span
          key={delay}
          className="aspect-square w-[15%] animate-suck rounded-full bg-current motion-reduce:animate-pulse"
          style={{ marginBottom: lift, animationDelay: delay }}
        />
      ))}
    </span>
  );
}

export { Spinner };
