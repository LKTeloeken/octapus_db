import {
  Add01Icon,
  ArrowDown01Icon,
  AiChat02Icon,
  Cancel01Icon,
  CommandIcon,
  FileCodeIcon,
  Search01Icon,
  SourceCodeIcon,
  TableIcon,
} from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { memo, useEffect, useState, type RefObject } from 'react';
import { ErrorBoundary } from '@/components/error-boundary';
import { Button } from '@/components/ui/button';
import {
  DropdownMenu,
  DropdownMenuContent,
  DropdownMenuItem,
  DropdownMenuTrigger,
} from '@/components/ui/dropdown-menu';
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
import { useIsTabRunning } from '@/stores/activity-store';
import { usePendingChangesStore } from '@/stores/pending-changes-store';
import type { WorkspaceTab } from '@/stores/tabs-store';
import { CloseTabDialog } from './close-tab-dialog';
import { useQueryTabs } from './use-query-tabs';

/** Ícone de cada tipo de aba (a cor é a do banco) */
const TAB_ICONS: Record<WorkspaceTab['kind'], typeof TableIcon> = {
  query: SourceCodeIcon,
  browse: TableIcon,
  definition: FileCodeIcon,
};

/**
 * O controle à direita da aba (DESIGN.md, Abas): × na ativa e no hover; nas
 * outras, uma ventosa diz o estado — biolume pulsando enquanto a consulta
 * roda, neutra quando há alterações não salvas. No hover a ventosa vira ×.
 */
function TabStatus({
  tabId,
  title,
  active,
  onClose,
}: {
  tabId: string;
  title: string;
  active: boolean;
  onClose: () => void;
}) {
  const running = useIsTabRunning(tabId);
  const dirty = usePendingChangesStore(state => state.byScope.has(tabId));
  const dot = running ? 'live' : dirty ? 'dirty' : null;

  return (
    <button
      type="button"
      data-tab-close
      aria-label={`Fechar aba ${title}${dirty ? ' (alterações não salvas)' : ''}${running ? ' (rodando)' : ''}`}
      className={cn(
        'group/close absolute top-1/2 right-1.5 z-10 inline-flex size-[18px] -translate-y-1/2 cursor-pointer items-center justify-center rounded-xs text-fg-subtle transition-[opacity,color,background-color] outline-none hover:bg-hover hover:text-fg focus-visible:opacity-100 focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-ring',
        active || dot ? 'opacity-100' : 'opacity-0 group-hover:opacity-100',
      )}
      onClick={onClose}
    >
      {dot && (
        <span
          aria-hidden
          className={cn(
            'size-[7px] rounded-full group-hover:hidden group-focus-visible/close:hidden',
            dot === 'live' ? 'animate-live bg-biolume' : 'bg-fg-muted',
          )}
        />
      )}
      <HugeiconsIcon
        icon={Cancel01Icon}
        className={cn(
          'size-3',
          dot && 'hidden group-hover:block group-focus-visible/close:block',
        )}
      />
    </button>
  );
}

/** A linha de abas transborda? (as que não cabem ficam no menu ▾) */
function useOverflowing(ref: RefObject<HTMLElement | null>, count: number) {
  const [overflowing, setOverflowing] = useState(false);
  useEffect(() => {
    const el = ref.current;
    if (!el) return;
    const check = () => setOverflowing(el.scrollWidth > el.clientWidth + 1);
    check();
    const observer = new ResizeObserver(check);
    observer.observe(el);
    return () => observer.disconnect();
  }, [ref, count]);
  return overflowing;
}

/** Lista de todas as abas, quando a linha não comporta todas (DESIGN.md, Abas) */
function TabOverflowMenu({
  tabs,
  activeTabId,
  listRef,
  onSelect,
}: {
  tabs: WorkspaceTab[];
  activeTabId: string | null;
  listRef: RefObject<HTMLDivElement | null>;
  onSelect: (id: string) => void;
}) {
  const overflowing = useOverflowing(listRef, tabs.length);
  if (!overflowing) return null;
  return (
    <DropdownMenu>
      <Tooltip content="Todas as abas" position="bottom">
        <DropdownMenuTrigger asChild>
          <Button variant="ghost" size="icon" aria-label="Todas as abas">
            <HugeiconsIcon icon={ArrowDown01Icon} />
          </Button>
        </DropdownMenuTrigger>
      </Tooltip>
      <DropdownMenuContent align="end" className="max-h-96 min-w-56">
        {tabs.map(tab => (
          <DropdownMenuItem
            key={tab.id}
            onSelect={() => onSelect(tab.id)}
            className={cn(tab.id === activeTabId && 'bg-active')}
          >
            <HugeiconsIcon
              icon={TAB_ICONS[tab.kind]}
              className="size-3.5 text-fg-subtle"
            />
            <span className="truncate">{tab.title}</span>
          </DropdownMenuItem>
        ))}
      </DropdownMenuContent>
    </DropdownMenu>
  );
}

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
        mascot
        icon={TableIcon}
        title="Nada por aqui ainda"
        description="Abra uma tabela na árvore ao lado ou busque por ela na paleta de comandos."
        action={
          <>
            <Button
              variant="ghost"
              size="sm"
              className="justify-between"
              onClick={openPalette}
            >
              <span className="flex items-center gap-2">
                <HugeiconsIcon icon={Search01Icon} />
                Buscar tabelas
              </span>
              {paletteKey && <Kbd>{paletteKey}</Kbd>}
            </Button>
            {assistantAvailable && !assistantOpen && (
              <Button
                variant="ghost"
                size="sm"
                className="justify-between"
                onClick={toggleAssistant}
              >
                <span className="flex items-center gap-2">
                  <HugeiconsIcon icon={AiChat02Icon} />
                  Perguntar ao assistente
                </span>
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
        className="@container flex h-[38px] shrink-0 items-center gap-1"
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
                      'pointer-events-none absolute top-1 bottom-1 z-20 w-0.5 rounded-full bg-tinta',
                      dropBefore ? '-left-0.5' : '-right-0.5',
                    )}
                  />
                )}
                <TabStatus
                  tabId={tab.id}
                  title={tab.title}
                  active={tab.id === activeTabId}
                  onClose={() => closeTab(tab.id)}
                />
              </div>
            );
          })}
        </TabsList>

        <TabOverflowMenu
          tabs={tabs}
          activeTabId={activeTabId}
          listRef={tabListRef}
          onSelect={setActiveTab}
        />

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
