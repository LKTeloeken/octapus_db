import {
  Add01Icon,
  Cancel01Icon,
  CommandIcon,
  Search01Icon,
  SourceCodeIcon,
  TableIcon,
} from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { memo } from 'react';
import { Button } from '@/components/ui/button';
import { ContentState } from '@/components/ui/content-state';
import { Kbd } from '@/components/ui/kbd';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Tooltip } from '@/components/ui/tooltip/tooltip';
import { QueryEditorPanel } from '@/features/query-editor/query-editor-panel';
import { TableBrowser } from '@/features/table-browser/table-browser';
import { DB_TYPE_TEXT_COLOR } from '@/lib/db-defaults';
import { shortcut } from '@/lib/platform';
import { cn } from '@/lib/utils';
import { useQueryTabs } from './use-query-tabs';

/** Tab bar + active tab content (browse → TableBrowser, query → QueryEditorPanel) */
export const QueryTabs = memo(() => {
  const {
    tabs,
    activeTab,
    activeTabId,
    dbTypeByServer,
    setActiveTab,
    closeTab,
    newQueryTab,
    openPalette,
  } = useQueryTabs();

  if (tabs.length === 0) {
    return (
      <ContentState
        icon={TableIcon}
        title="Nenhuma aba aberta"
        description="Abra uma tabela na árvore ao lado ou busque por ela na paleta de comandos."
        action={
          <Button variant="outline" size="sm" onClick={openPalette}>
            <HugeiconsIcon icon={Search01Icon} />
            Buscar tabelas
            <Kbd>{shortcut('K')}</Kbd>
          </Button>
        }
      />
    );
  }

  return (
    <Tabs
      value={activeTabId ?? ''}
      onValueChange={setActiveTab}
      className="h-full w-full flex flex-col gap-1.5"
    >
      <div className="flex h-9 shrink-0 items-center gap-1">
        <TabsList
          variant="document"
          className="min-w-0 shrink justify-start overflow-x-auto overflow-y-hidden no-scrollbar px-0.5"
        >
          {tabs.map(tab => {
            const dbType = dbTypeByServer.get(tab.serverId);

            return (
              <div
                key={tab.id}
                className="group relative shrink-0"
                onMouseDown={event => {
                  if (event.button === 1) event.preventDefault();
                }}
                onAuxClick={event => {
                  if (event.button === 1) closeTab(tab.id);
                }}
              >
                <TabsTrigger value={tab.id} className="gap-[7px] pr-7">
                  <HugeiconsIcon
                    icon={tab.kind === 'browse' ? TableIcon : SourceCodeIcon}
                    className={cn(
                      'size-[15px]',
                      dbType ? DB_TYPE_TEXT_COLOR[dbType] : 'text-fg-subtle',
                    )}
                  />
                  <span className="max-w-48 truncate">{tab.title}</span>
                </TabsTrigger>
                <button
                  type="button"
                  aria-label={`Fechar aba ${tab.title}`}
                  className={cn(
                    'absolute top-1/2 right-1.5 z-10 inline-flex size-[18px] -translate-y-1/2 cursor-pointer items-center justify-center rounded-xs text-fg-subtle transition-[opacity,color,background-color] outline-none hover:bg-hover hover:text-fg focus-visible:opacity-100 focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-ring',
                    tab.id === activeTabId
                      ? 'opacity-100'
                      : 'opacity-0 group-hover:opacity-100',
                  )}
                  onClick={() => closeTab(tab.id)}
                >
                  <HugeiconsIcon icon={Cancel01Icon} className="size-3" />
                </button>
              </div>
            );
          })}
        </TabsList>

        <Tooltip content="Nova consulta neste banco" position="bottom">
          <Button
            variant="ghost"
            size="icon"
            aria-label="Nova consulta"
            onClick={newQueryTab}
            disabled={!activeTab}
          >
            <HugeiconsIcon icon={Add01Icon} />
          </Button>
        </Tooltip>

        <span className="flex-1" />

        <Button
          variant="ghost"
          size="sm"
          className="shrink-0 gap-2 border border-line pr-1.5 pl-2.5 text-small text-fg-subtle"
          onClick={openPalette}
        >
          <HugeiconsIcon icon={CommandIcon} className="size-3.5" />
          Comandos
          <Kbd>{shortcut('K')}</Kbd>
        </Button>
      </div>

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
