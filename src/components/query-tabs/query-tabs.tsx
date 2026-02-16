import { memo } from 'react';
import { Tabs, TabsList, TabsTrigger, TabsContent } from '@/components/ui/tabs';
import { Database, X } from 'lucide-react';

import type { QueryTabsProps } from './query-tabs.types';
import { cn } from '@/lib/utils';

export const QueryTabs = memo(
  ({
    tabs,
    activeTabId,
    onTabChange,
    onTabClose,
    children,
  }: QueryTabsProps) => {
    return (
      <Tabs
        defaultValue={activeTabId}
        value={activeTabId}
        onValueChange={onTabChange}
        className="h-full w-full flex flex-col"
      >
        <TabsList className="p-1 shrink-0">
          {tabs.map(tab => (
            <div key={tab.id} className="relative">
              <TabsTrigger value={tab.id} className="px-2.5 pr-7 sm:px-3 sm:pr-7">
                <code className="flex items-center gap-1 text-[13px] [&>svg]:h-4 [&>svg]:w-4">
                  <Database
                    className={cn(tab.id === activeTabId && 'text-primary')}
                  />{' '}
                  {tab.title}
                </code>
              </TabsTrigger>
              <span
                className="absolute right-1.5 top-1/2 -translate-y-1/2 z-10 inline-flex h-4 w-4 items-center justify-center rounded-sm hover:bg-accent hover:text-accent-foreground cursor-pointer"
                onClick={() => onTabClose(tab.id)}
              >
                <X className="h-3 w-3" />
              </span>
            </div>
          ))}
        </TabsList>

        {activeTabId && (
          <TabsContent
            value={activeTabId}
            className="flex-1 overflow-hidden mt-0"
          >
            {children}
          </TabsContent>
        )}
      </Tabs>
    );
  },
);
