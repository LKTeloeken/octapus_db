import * as React from 'react';
import * as TabsPrimitive from '@radix-ui/react-tabs';

import { cn } from '@/lib/utils';

function Tabs({
  className,
  ...props
}: React.ComponentProps<typeof TabsPrimitive.Root>) {
  return (
    <TabsPrimitive.Root
      data-slot="tabs"
      className={cn('flex flex-col gap-1 border-none!', className)}
      {...props}
    />
  );
}

function TabsList({
  className,
  ...props
}: React.ComponentProps<typeof TabsPrimitive.List>) {
  return (
    <TabsPrimitive.List
      data-slot="tabs-list"
      className={cn(
        'bg-sidebar border border-border rounded-md text-muted-foreground inline-flex w-fit items-center justify-center p-[3px]',
        className,
      )}
      {...props}
    />
  );
}

function TabsTrigger({
  className,
  ...props
}: React.ComponentProps<typeof TabsPrimitive.Trigger>) {
  return (
    <TabsPrimitive.Trigger
      data-slot="tabs-trigger"
      className={cn(
        // Layout & spacing
        'inline-flex flex-1 items-center justify-center h-[calc(100%-1px)] gap-1.5 rounded-sm px-2 py-1 whitespace-nowrap cursor-pointer hover:bg-muted/20 transition-colors',

        // Typography
        'text-sm font-medium text-foreground dark:text-muted-foreground',

        // Border
        'border border-border data-[state=inactive]:border-dashed',

        // State: active
        'data-[state=active]:bg-background dark:data-[state=active]:text-foreground data-[state=active]:shadow-sm dark:data-[state=active]:border-border dark:data-[state=active]:bg-input/30',

        // Focus/outline
        'focus-visible:border-ring focus-visible:outline-ring focus-visible:ring-ring/50 focus-visible:ring-[3px] focus-visible:outline-1',

        // Disabled
        'disabled:pointer-events-none disabled:opacity-50',

        // Transition
        'transition-[color,box-shadow]',

        // SVG styling
        "[&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",

        className,
      )}
      {...props}
    />
  );
}

function TabsContent({
  className,
  ...props
}: React.ComponentProps<typeof TabsPrimitive.Content>) {
  return (
    <TabsPrimitive.Content
      data-slot="tabs-content"
      className={cn('flex-1 outline-none', className)}
      {...props}
    />
  );
}

export { Tabs, TabsList, TabsTrigger, TabsContent };
