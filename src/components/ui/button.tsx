import * as React from 'react';
import { Slot } from '@radix-ui/react-slot';
import { cva, type VariantProps } from 'class-variance-authority';

import { cn } from '@/lib/utils';

/**
 * Botões do Ink (DESIGN.md §8). `default` é o primário em Iris — um por área;
 * `outline`/`secondary` são o neutro com borda; `ghost` não tem fundo;
 * `destructive` é o perigo em tom suave, sólido só no hover. Um foco só
 * (anel em `--ring` com 2 px de respiro, só no teclado) e press em 98%.
 */
const buttonVariants = cva(
  [
    'inline-flex shrink-0 cursor-pointer items-center justify-center whitespace-nowrap select-none font-medium leading-none',
    'transition-[background-color,color,box-shadow,transform] ease-standard active:scale-[0.98] active:duration-[80ms]',
    'outline-none focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-offset-2 focus-visible:outline-ring',
    'disabled:pointer-events-none disabled:opacity-45 aria-invalid:border-danger',
    "[&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
  ],
  {
    variants: {
      variant: {
        default: 'bg-iris text-white shadow-cta hover:bg-iris-hover',
        outline:
          'border border-control-border bg-control text-fg shadow-control hover:bg-active data-[state=open]:bg-active',
        secondary:
          'border border-control-border bg-control text-fg shadow-control hover:bg-active data-[state=open]:bg-active',
        ghost:
          'text-fg-muted hover:bg-hover hover:text-fg data-[state=open]:bg-active data-[state=open]:text-fg aria-pressed:bg-active aria-pressed:text-fg',
        destructive:
          'bg-danger-soft text-danger hover:bg-danger hover:text-white',
        link: 'text-iris-text underline-offset-4 hover:underline',
      },
      size: {
        xs: "h-6 gap-1 rounded-sm px-2 text-small [&_svg:not([class*='size-'])]:size-3.5",
        sm: 'h-7 gap-1.5 rounded-sm px-2.5 text-body',
        default: 'h-8 gap-1.5 rounded-md px-3 text-body',
        lg: 'h-9 gap-2 rounded-md px-3.5 text-heading',
        icon: 'size-7 rounded-sm',
        'icon-xs': "size-6 rounded-sm [&_svg:not([class*='size-'])]:size-3.5",
      },
    },
    defaultVariants: {
      variant: 'default',
      size: 'sm',
    },
  },
);

export interface ButtonProps
  extends React.ComponentProps<'button'>, VariantProps<typeof buttonVariants> {
  asChild?: boolean;
}

function Button({
  className,
  variant,
  size,
  asChild = false,
  ...props
}: ButtonProps) {
  const Comp = asChild ? Slot : 'button';

  return (
    <Comp
      data-slot="button"
      className={cn(buttonVariants({ variant, size, className }))}
      {...props}
    />
  );
}

export { Button, buttonVariants };
