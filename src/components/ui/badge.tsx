import * as React from 'react';
import { Slot } from '@radix-ui/react-slot';
import { cva, type VariantProps } from 'class-variance-authority';

import { cn } from '@/lib/utils';

/**
 * Marcador de 20 px, 11 px/500, fundo `-soft` + texto sólido do tom. Os nomes
 * antigos do shadcn continuam como apelidos: `default` → accent,
 * `secondary`/`outline` → neutral, `destructive` → danger.
 */
const badgeVariants = cva(
  'inline-flex h-5 w-fit shrink-0 items-center justify-center gap-1 overflow-hidden whitespace-nowrap rounded-[5px] px-1.5 text-[11px] leading-none font-medium transition-colors [&>svg]:pointer-events-none [&>svg]:size-3',
  {
    variants: {
      variant: {
        neutral: 'bg-hover text-fg-muted',
        accent: 'bg-iris-soft text-iris-text',
        success: 'bg-success-soft text-success',
        warning: 'bg-warning-soft text-warning',
        danger: 'bg-danger-soft text-danger',
        info: 'bg-info-soft text-info',
        default: 'bg-iris-soft text-iris-text',
        secondary: 'bg-hover text-fg-muted',
        outline: 'bg-hover text-fg-muted',
        destructive: 'bg-danger-soft text-danger',
      },
      mono: {
        true: 'font-mono',
        false: '',
      },
    },
    defaultVariants: {
      variant: 'neutral',
      mono: false,
    },
  },
);

function Badge({
  className,
  variant,
  mono,
  asChild = false,
  ...props
}: React.ComponentProps<'span'> &
  VariantProps<typeof badgeVariants> & { asChild?: boolean }) {
  const Comp = asChild ? Slot : 'span';

  return (
    <Comp
      data-slot="badge"
      className={cn(badgeVariants({ variant, mono }), className)}
      {...props}
    />
  );
}

export { Badge, badgeVariants };
