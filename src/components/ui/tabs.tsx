import * as React from 'react';
import * as TabsPrimitive from '@radix-ui/react-tabs';

import { cn } from '@/lib/utils';

/**
 * Duas formas de aba (DESIGN.md §8):
 * - `document` — barra de abas abertas: a ativa sobe de camada
 *   (`--surface-1` + `shadow-raised`), sem violeta;
 * - `view` — alternância de visões (Resultados/Mensagens): sublinhado neutro.
 * A variante é do `TabsList` e chega aos triggers por contexto.
 */
type TabsVariant = 'document' | 'view';

const TabsVariantContext = React.createContext<TabsVariant>('view');

function Tabs({
  className,
  ...props
}: React.ComponentProps<typeof TabsPrimitive.Root>) {
  return (
    <TabsPrimitive.Root
      data-slot="tabs"
      className={cn('flex flex-col gap-1', className)}
      {...props}
    />
  );
}

const LIST_CLASS: Record<TabsVariant, string> = {
  document: 'inline-flex h-9 w-fit items-center gap-1',
  view: 'inline-flex h-10 w-fit items-center gap-[18px]',
};

function TabsList({
  className,
  variant = 'view',
  ...props
}: React.ComponentProps<typeof TabsPrimitive.List> & {
  variant?: TabsVariant;
}) {
  return (
    <TabsVariantContext.Provider value={variant}>
      <TabsPrimitive.List
        data-slot="tabs-list"
        data-variant={variant}
        className={cn(LIST_CLASS[variant], className)}
        {...props}
      />
    </TabsVariantContext.Provider>
  );
}

const TRIGGER_BASE = [
  'inline-flex shrink-0 cursor-pointer items-center justify-center gap-1.5 whitespace-nowrap text-body font-medium text-fg-muted',
  'transition-[color,background-color,box-shadow] hover:text-fg',
  'outline-none focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-offset-2 focus-visible:outline-ring',
  'disabled:pointer-events-none disabled:opacity-45',
  "[&_svg]:pointer-events-none [&_svg]:shrink-0 [&_svg:not([class*='size-'])]:size-4",
].join(' ');

const TRIGGER_CLASS: Record<TabsVariant, string> = {
  document:
    'h-[30px] rounded-md px-2.5 hover:bg-hover data-[state=active]:bg-surface-1 data-[state=active]:text-fg data-[state=active]:shadow-raised data-[state=active]:ring-1 data-[state=active]:ring-inset data-[state=active]:ring-line',
  view: 'h-10 px-0.5 data-[state=active]:text-fg data-[state=active]:shadow-[inset_0_-2px_0_var(--fg)]',
};

function TabsTrigger({
  className,
  ...props
}: React.ComponentProps<typeof TabsPrimitive.Trigger>) {
  const variant = React.useContext(TabsVariantContext);

  return (
    <TabsPrimitive.Trigger
      data-slot="tabs-trigger"
      className={cn(TRIGGER_BASE, TRIGGER_CLASS[variant], className)}
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
