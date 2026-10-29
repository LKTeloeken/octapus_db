import {
  Add01Icon,
  AiChat02Icon,
  Cancel01Icon,
  CommandIcon,
  FileCodeIcon,
  Search01Icon,
  SourceCodeIcon,
  TableIcon,
} from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { memo } from 'react';
import { ErrorBoundary } from '@/components/error-boundary';
import { Button } from '@/components/ui/button';
import { ContentState } from '@/components/ui/content-state';
import { Kbd } from '@/components/ui/kbd';
import { Separator } from '@/components/ui/separator';
import { Tabs, TabsContent, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Tooltip } from '@/components/ui/tooltip/tooltip';
import { DefinitionTab } from '@/features/definition-viewer/definition-tab';
import { QueryEditorPanel } from '@/features/query-editor/query-editor-panel';
import { TableBrowser } from '@/features/table-browser/table-browser';
import { WindowControls } from '@/features/window-controls/window-controls';
import { DB_TYPE_TEXT_COLOR } from '@/lib/db-defaults';
import { customTitlebar } from '@/lib/platform';
import {
  useShortcutLabel,
  withShortcut,
} from '@/lib/keybindings/use-shortcuts';
import { cn } from '@/lib/utils';
import type { WorkspaceTab } from '@/stores/tabs-store';
import { CloseTabDialog } from './close-tab-dialog';
import { useQueryTabs } from './use-query-tabs';

/** Ícone de cada tipo de aba (a cor é a do banco) */
const TAB_ICONS: Record<WorkspaceTab['kind'], typeof TableIcon> = {
  query: SourceCodeIcon,
  browse: TableIcon,
  definition: FileCodeIcon,
};

/** Assistente e Comandos; na barra estreita só o ícone (padding simétrico) */
const HEADER_BUTTON_CLASS =
  'shrink-0 gap-2 border border-line pr-1.5 pl-2.5 text-small text-fg-subtle @max-2xl:px-2';

/** Tab bar + active tab content (browse → TableBrowser, query → QueryEditorPanel) */
export const QueryTabs = memo(() => {
  const {
    tabs,
    tabListRef,
    activeTab,
    activeTabId,
    dbTypeByServer,
    descriptions,
    locationOf,
    drag,
    onTabPointerDown,
    setActiveTab,
    closeTab,
    closing,
    confirmClose,
    cancelClose,
    newQueryTab,
    openPalette,
    assistantAvailable,
    assistantOpen,
    toggleAssistant,
  } = useQueryTabs();
  const paletteKey = useShortcutLabel('app.commandPalette');
  const assistantKey = useShortcutLabel('assistant.togglePanel');

  const assistantButton = assistantAvailable && (
    <Tooltip
      content={withShortcut('Assistente', assistantKey)}
      position="bottom"
    >
      <Button
        variant="ghost"
        size="sm"
        aria-pressed={assistantOpen}
        className={HEADER_BUTTON_CLASS}
        onClick={toggleAssistant}
      >
        <HugeiconsIcon icon={AiChat02Icon} className="size-3.5" />
        <span className="@max-2xl:sr-only">Assistente</span>
        {assistantKey && <Kbd className="@max-2xl:hidden">{assistantKey}</Kbd>}
      </Button>
    </Tooltip>
  );

  // Posição da aba arrastada, para não marcar onde soltar não muda nada
  const dragFrom = drag ? tabs.findIndex(item => item.id === drag.tabId) : -1;

  if (tabs.length === 0) {
    const emptyState = (
      <ContentState
        icon={TableIcon}
        title="Nenhuma aba aberta"
        description="Abra uma tabela na árvore ao lado ou busque por ela na paleta de comandos."
        action={
          <>
            <Button variant="outline" size="sm" onClick={openPalette}>
              <HugeiconsIcon icon={Search01Icon} />
              Buscar tabelas
              {paletteKey && <Kbd>{paletteKey}</Kbd>}
            </Button>
            {assistantAvailable && !assistantOpen && (
              <Button variant="outline" size="sm" onClick={toggleAssistant}>
                <HugeiconsIcon icon={AiChat02Icon} />
                Perguntar ao assistente
                {assistantKey && <Kbd>{assistantKey}</Kbd>}
              </Button>
            )}
          </>
        }
      />
    );

    // Sem abas não há linha de abas: no Linux os controles ganham uma faixa
    // própria, senão a janela ficaria sem como fechar/maximizar.
    if (customTitlebar !== 'linux') return emptyState;

    return (
      <div className="flex h-full w-full flex-col gap-1.5">
        <div
          data-tauri-drag-region="deep"
          className="flex h-9 shrink-0 items-center justify-end"
        >
          <WindowControls />
        </div>
        <div className="min-h-0 flex-1">{emptyState}</div>
      </div>
    );
  }

  return (
    <Tabs
      value={activeTabId ?? ''}
      onValueChange={setActiveTab}
      className="h-full w-full flex flex-col gap-1.5"
    >
      {/* Também é a barra de título (ver Sidebar): o espaço livre arrasta.
          Estreita (container query), Assistente e Comandos ficam só com o
          ícone e as abas ganham o espaço. */}
      <div
        data-tauri-drag-region="deep"
        className="@container flex h-9 shrink-0 items-center gap-1"
      >
        <TabsList
          ref={tabListRef}
          variant="document"
          className="min-w-0 shrink justify-start overflow-x-auto overflow-y-hidden no-scrollbar px-0.5"
        >
          {tabs.map((tab, index) => {
            const dbType = dbTypeByServer.get(tab.serverId);
            const description = descriptions.get(tab.id);
            // Marca de inserção à esquerda desta aba (ou à direita da última),
            // fora das posições em que soltar não muda nada
            const dropHere =
              drag !== null &&
              drag.dropIndex !== dragFrom &&
              drag.dropIndex !== dragFrom + 1;
            const dropBefore = dropHere && drag.dropIndex === index;
            const dropAfter =
              dropHere &&
              index === tabs.length - 1 &&
              drag.dropIndex === tabs.length;

            return (
              <div
                key={tab.id}
                data-tab-id={tab.id}
                title={locationOf(tab)}
                className={cn(
                  'group relative shrink-0',
                  drag?.tabId === tab.id && 'opacity-50',
                )}
                onPointerDown={event => onTabPointerDown(tab.id, event)}
                onMouseDown={event => {
                  if (event.button === 1) event.preventDefault();
                }}
                onAuxClick={event => {
                  if (event.button === 1) closeTab(tab.id);
                }}
              >
                <TabsTrigger value={tab.id} className="gap-[7px] pr-7">
                  <HugeiconsIcon
                    icon={TAB_ICONS[tab.kind]}
                    className={cn(
                      'size-[15px]',
                      dbType ? DB_TYPE_TEXT_COLOR[dbType] : 'text-fg-subtle',
                    )}
                  />
                  <span className="max-w-48 truncate">{tab.title}</span>
                  {description && (
                    <span className="max-w-32 truncate text-small text-fg-subtle">
                      {/* Leitor de tela: "events em tenant_00001" */}
                      <span className="sr-only"> em </span>
                      {description}
                    </span>
                  )}
                </TabsTrigger>
                {(dropBefore || dropAfter) && (
                  <span
                    aria-hidden
                    className={cn(
                      'pointer-events-none absolute top-1 bottom-1 z-20 w-0.5 rounded-full bg-iris',
                      dropBefore ? '-left-0.5' : '-right-0.5',
                    )}
                  />
                )}
                <button
                  type="button"
                  data-tab-close
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

        {assistantButton}

        <Tooltip
          content={withShortcut('Comandos', paletteKey)}
          position="bottom"
        >
          <Button
            variant="ghost"
            size="sm"
            className={HEADER_BUTTON_CLASS}
            onClick={openPalette}
          >
            <HugeiconsIcon icon={CommandIcon} className="size-3.5" />
            <span className="@max-2xl:sr-only">Comandos</span>
            {paletteKey && <Kbd className="@max-2xl:hidden">{paletteKey}</Kbd>}
          </Button>
        </Tooltip>

        {customTitlebar === 'linux' && (
          <>
            <Separator
              orientation="vertical"
              className="mx-1 h-[18px]! bg-line"
            />
            <WindowControls />
          </>
        )}
      </div>

      {activeTab && (
        <TabsContent
          value={activeTab.id}
          className="flex-1 overflow-hidden mt-0"
        >
          {/* Erro de render numa aba fica nela; trocar de aba limpa */}
          <ErrorBoundary area="nesta aba" resetKeys={[activeTab.id]}>
            {activeTab.kind === 'browse' ? (
              <TableBrowser tab={activeTab} />
            ) : activeTab.kind === 'definition' ? (
              <DefinitionTab tab={activeTab} />
            ) : (
              <QueryEditorPanel tab={activeTab} />
            )}
          </ErrorBoundary>
        </TabsContent>
      )}

      <CloseTabDialog
        closing={closing}
        onConfirm={confirmClose}
        onCancel={cancelClose}
      />
    </Tabs>
  );
});

QueryTabs.displayName = 'QueryTabs';
