import { HugeiconsIcon } from '@hugeicons/react';
import { MinusSignIcon, Tick01Icon } from '@hugeicons/core-free-icons';
import * as React from 'react';
import * as CheckboxPrimitive from '@radix-ui/react-checkbox';
import { cn } from '@/lib/utils';

/**
 * Controle ligado é um dos papéis de Iris. `indeterminate` (o NULL das
 * colunas booleanas) mostra um traço em vez do ✓.
 */
function Checkbox({
  className,
  ...props
}: React.ComponentProps<typeof CheckboxPrimitive.Root>) {
  return (
    <CheckboxPrimitive.Root
      data-slot="checkbox"
      className={cn(
        'peer size-4 shrink-0 cursor-pointer rounded-xs border-[1.5px] border-line-strong bg-transparent text-white transition-colors',
        'data-[state=checked]:border-iris data-[state=checked]:bg-iris data-[state=indeterminate]:border-iris data-[state=indeterminate]:bg-iris',
        'outline-none focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-offset-2 focus-visible:outline-ring',
        'aria-invalid:border-danger disabled:cursor-not-allowed disabled:opacity-50',
        className,
      )}
      {...props}
    >
      <CheckboxPrimitive.Indicator
        data-slot="checkbox-indicator"
        className="group/indicator flex items-center justify-center text-current transition-none"
      >
        <HugeiconsIcon
          icon={Tick01Icon}
          strokeWidth={2.5}
          className="size-3 group-data-[state=indeterminate]/indicator:hidden"
        />
        <HugeiconsIcon
          icon={MinusSignIcon}
          strokeWidth={2.5}
          className="hidden size-3 group-data-[state=indeterminate]/indicator:block"
        />
      </CheckboxPrimitive.Indicator>
    </CheckboxPrimitive.Root>
  );
}

export { Checkbox };
