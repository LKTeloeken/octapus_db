import * as React from 'react';
import * as SwitchPrimitive from '@radix-ui/react-switch';

import { cn } from '@/lib/utils';

/** 28×16; ligado em Tinta, o polegar desliza com `ease-spring`. */
function Switch({
  className,
  ...props
}: React.ComponentProps<typeof SwitchPrimitive.Root>) {
  return (
    <SwitchPrimitive.Root
      data-slot="switch"
      className={cn(
        'peer inline-flex h-4 w-7 shrink-0 cursor-pointer items-center rounded-full p-0.5 transition-colors',
        'data-[state=checked]:bg-tinta data-[state=unchecked]:bg-edge-control',
        'outline-none focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-offset-2 focus-visible:outline-ring',
        'disabled:cursor-not-allowed disabled:opacity-50',
        className,
      )}
      {...props}
    >
      <SwitchPrimitive.Thumb
        data-slot="switch-thumb"
        // Indicador em `--on-tinta` ligado e `--surface-3` desligado
        className="pointer-events-none block size-3 rounded-full bg-surface-3 shadow-raised data-[state=checked]:bg-on-tinta transition-transform duration-180 ease-spring data-[state=checked]:translate-x-3 data-[state=unchecked]:translate-x-0 motion-reduce:transition-none"
      />
    </SwitchPrimitive.Root>
  );
}

export { Switch };
