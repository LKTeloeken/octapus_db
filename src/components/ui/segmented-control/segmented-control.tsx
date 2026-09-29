import { cn } from '@/lib/utils';
import type { SegmentedControlProps } from './segmented-control.types';
import { useSegmentedControl } from './use-segmented-control';

/**
 * Escolha exclusiva entre poucas opções (Tabela/Vertical). Trilho `--hover`,
 * indicador elevado que desliza entre os itens em 180 ms com `ease-spring`.
 */
export function SegmentedControl<T extends string>({
  value,
  onValueChange,
  options,
  size = 'xs',
  className,
  'aria-label': ariaLabel,
}: SegmentedControlProps<T>) {
  const { thumb, isReady, setItemRef, onKeyDown } = useSegmentedControl({
    value,
    onValueChange,
    options,
  });

  return (
    <div
      role="radiogroup"
      aria-label={ariaLabel}
      data-slot="segmented-control"
      onKeyDown={onKeyDown}
      className={cn(
        'relative inline-flex shrink-0 gap-0.5 rounded-[7px] bg-hover p-0.5 shadow-[inset_0_0_0_1px_var(--line-subtle)]',
        className,
      )}
    >
      {thumb && (
        <span
          aria-hidden
          className={cn(
            'absolute top-0.5 bottom-0.5 left-0 rounded-[5px] bg-surface-3 shadow-raised',
            isReady &&
              'transition-[transform,width] duration-180 ease-spring motion-reduce:transition-none',
          )}
          style={{
            width: `${thumb.width}px`,
            transform: `translateX(${thumb.left}px)`,
          }}
        />
      )}

      {options.map(option => {
        const isActive = option.value === value;

        return (
          <button
            key={option.value}
            ref={setItemRef(option.value)}
            type="button"
            role="radio"
            aria-checked={isActive}
            // Um único tab stop: as setas movem entre as opções.
            tabIndex={isActive ? 0 : -1}
            onClick={() => onValueChange(option.value)}
            className={cn(
              'relative z-10 inline-flex cursor-pointer items-center gap-1.5 rounded-[5px] px-2.5 text-small font-medium transition-colors outline-none focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-offset-1 focus-visible:outline-ring',
              size === 'xs' ? 'h-[18px]' : 'h-6',
              isActive ? 'text-fg' : 'text-fg-muted hover:text-fg',
            )}
          >
            {option.label}
          </button>
        );
      })}
    </div>
  );
}
