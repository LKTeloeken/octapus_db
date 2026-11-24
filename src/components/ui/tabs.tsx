import * as React from 'react';
import * as TabsPrimitive from '@radix-ui/react-tabs';

import { cn } from '@/lib/utils';

/**
 * Duas formas de aba (DESIGN.md, Abas):
 * - `document` — barra de abas abertas: a ativa sobe de camada
 *   (`--surface-1` + `shadow-raised`) e ganha o tentáculo: uma pílula de
 *   2 px em `--tinta` no topo;
 * - `view` — alternância de visões (Resultados/Mensagens): sublinhado neutro
 *   em pílula.
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
  document: [
    'relative h-8 rounded-md px-2.5 hover:bg-hover data-[state=active]:bg-surface-1 data-[state=active]:text-fg data-[state=active]:shadow-raised data-[state=active]:ring-1 data-[state=active]:ring-inset data-[state=active]:ring-line',
    // Tentáculo: aparece em 180 ms na aba que fica ativa
    'before:pointer-events-none before:absolute before:inset-x-2.5 before:top-0 before:h-0.5 before:rounded-b-full before:bg-tinta before:opacity-0 before:transition-opacity before:duration-180 data-[state=active]:before:opacity-100',
  ].join(' '),
  view: [
    'relative h-10 px-0.5 data-[state=active]:text-fg',
    'after:pointer-events-none after:absolute after:inset-x-0 after:bottom-0 after:h-0.5 after:rounded-full after:bg-fg after:opacity-0 after:transition-opacity data-[state=active]:after:opacity-100',
  ].join(' '),
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
