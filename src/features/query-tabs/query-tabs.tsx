import {
  Cancel01Icon,
  DatabaseIcon,
  TableIcon,
} from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { memo } from 'react';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Typography } from '@/components/ui/typography';
import { QueryEditorPanel } from '@/features/query-editor/query-editor-panel';
import { TableBrowser } from '@/features/table-browser/table-browser';
import { cn } from '@/lib/utils';
import { useQueryTabs } from './use-query-tabs';

/** Tab bar + active tab content (browse → TableBrowser, query → QueryEditorPanel) */
export const QueryTabs = memo(() => {
  const { tabs, activeTab, activeTabId, setActiveTab, closeTab } =
    useQueryTabs();

  if (tabs.length === 0) {
    return (
      <div className="flex h-full items-center justify-center">
        <Typography variant="p" className="text-muted-foreground">
          Abra uma tabela ou crie uma nova aba para começar
        </Typography>
      </div>
    );
  }

  return (
    <Tabs
      value={activeTabId ?? ''}
      onValueChange={setActiveTab}
      className="h-full w-full flex flex-col gap-2"
    >
      <TabsList className="justify-start p-2 w-full gap-2 overflow-x-auto overflow-y-hidden no-scrollbar">
        {tabs.map(tab => (
          <div
            key={tab.id}
            className="relative"
            // Botão do meio fecha a aba (padrão de navegadores). O preventDefault
            // no mousedown evita o autoscroll do middle-click no Windows/Linux.
            onMouseDown={event => {
              if (event.button === 1) event.preventDefault();
            }}
            onAuxClick={event => {
              if (event.button === 1) closeTab(tab.id);
            }}
          >
            <TabsTrigger value={tab.id} className="px-2.5 pr-7 sm:px-3 sm:pr-7">
              <code className="flex items-center gap-1 text-[13px] [&>svg]:h-4 [&>svg]:w-4">
                <HugeiconsIcon
                  icon={tab.kind === 'browse' ? TableIcon : DatabaseIcon}
                  className={cn(tab.id === activeTabId && 'text-primary')}
                />{' '}
                {tab.title}
              </code>
            </TabsTrigger>
            <span
              className="absolute right-1.5 top-1/2 -translate-y-1/2 z-10 inline-flex h-4 w-4 items-center justify-center rounded-sm hover:bg-accent hover:text-accent-foreground cursor-pointer"
              onClick={() => closeTab(tab.id)}
            >
              <HugeiconsIcon icon={Cancel01Icon} className="h-3 w-3" />
            </span>
          </div>
        ))}
      </TabsList>

      {activeTab && (
        <TabsContent
          value={activeTab.id}
          className="flex-1 overflow-hidden mt-0"
        >
          {activeTab.kind === 'browse' ? (
            <TableBrowser tab={activeTab} />
          ) : (
            <QueryEditorPanel tab={activeTab} />
          )}
        </TabsContent>
      )}
    </Tabs>
  );
});

QueryTabs.displayName = 'QueryTabs';
