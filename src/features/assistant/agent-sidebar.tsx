import { Add01Icon, Cancel01Icon } from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { Button } from '@/components/ui/button';
import { Kbd } from '@/components/ui/kbd';
import { Spinner } from '@/components/ui/spinner';
import { OctapusThinking } from '@/components/octapus-thinking/octapus-thinking';
import { useShortcutLabel } from '@/lib/keybindings/use-shortcuts';
import { cn } from '@/lib/utils';
import { conversationDetail } from './conversation-labels';
import { useConversationHistory } from './use-history-menu';

export interface AgentSidebarTab {
  id: string;
  title: string;
  running: boolean;
  conversationId: string | null;
}

interface AgentSidebarProps {
  tabs: AgentSidebarTab[];
  activeTabId: string | null;
  onSelect: (id: string) => void;
  onClose: (id: string) => void;
  onNew: () => void;
}

const row =
  'flex w-full cursor-pointer flex-col items-start gap-0.5 rounded-sm px-2 py-1.5 text-left outline-none transition-colors duration-120 hover:bg-hover focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-ring';

/**
 * Modo agente: as conversas abertas (as abas do painel) e as anteriores, do
 * servidor, numa coluna à esquerda.
 */
export function AgentSidebar({
  tabs,
  activeTabId,
  onSelect,
  onClose,
  onNew,
}: AgentSidebarProps) {
  const newKey = useShortcutLabel('assistant.newConversation');
  const history = useConversationHistory(true);
  const open = new Set(tabs.map(tab => tab.conversationId));
  const previous = history.items.filter(item => !open.has(item.id));

  return (
    <aside
      aria-label="Conversas"
      className="flex w-64 shrink-0 flex-col border-r border-line-subtle"
    >
      <div className="p-2">
        <Button
          variant="outline"
          size="sm"
          className="w-full justify-start"
          onClick={onNew}
        >
          <HugeiconsIcon icon={Add01Icon} />
          <span className="flex-1 text-left">Nova conversa</span>
          {newKey && <Kbd>{newKey}</Kbd>}
        </Button>
      </div>

      <div className="min-h-0 flex-1 overflow-y-auto px-1.5 pb-2 scrollbar-thin">
        {tabs.length > 0 && (
          <>
            <p className="px-2 pt-1.5 pb-1 text-micro font-medium uppercase text-fg-subtle">
              Abertas
            </p>
            <ul className="flex flex-col">
              {tabs.map(tab => (
                <li key={tab.id} className="group relative">
                  <button
                    type="button"
                    aria-current={tab.id === activeTabId ? 'true' : undefined}
                    onClick={() => onSelect(tab.id)}
                    className={cn(
                      row,
                      'pr-8',
                      tab.id === activeTabId && 'bg-active hover:bg-active',
                    )}
                  >
                    <span className="flex w-full items-center gap-1.5 text-body text-fg">
                      {tab.running && (
                        <OctapusThinking className="size-3.5 shrink-0" />
                      )}
                      <span className="truncate">{tab.title}</span>
                    </span>
                  </button>
                  <button
                    type="button"
                    aria-label={`Fechar conversa ${tab.title}`}
                    onClick={() => onClose(tab.id)}
                    className={cn(
                      'absolute top-1/2 right-1.5 inline-flex size-[18px] -translate-y-1/2 cursor-pointer items-center justify-center rounded-xs text-fg-subtle transition-[opacity,color,background-color] duration-120 outline-none hover:bg-hover hover:text-fg focus-visible:opacity-100 focus-visible:outline-2 focus-visible:outline-solid focus-visible:outline-ring',
                      tab.id === activeTabId
                        ? 'opacity-100'
                        : 'opacity-0 group-hover:opacity-100',
                    )}
                  >
                    <HugeiconsIcon icon={Cancel01Icon} className="size-3" />
                  </button>
                </li>
              ))}
            </ul>
          </>
        )}

        <p className="px-2 pt-3 pb-1 text-micro font-medium uppercase text-fg-subtle">
          Anteriores
        </p>
        {history.isLoading ? (
          <div className="flex justify-center py-4">
            <Spinner className="size-4 text-fg-subtle" />
          </div>
        ) : history.error ? (
          <p className="px-2 py-1.5 text-small text-danger">
            Não consegui carregar o histórico.
          </p>
        ) : previous.length === 0 ? (
          <p className="px-2 py-1.5 text-small text-fg-muted">
            Nenhuma outra conversa.
          </p>
        ) : (
          <ul className="flex flex-col">
            {previous.map(item => (
              <li key={item.id}>
                <button
                  type="button"
                  onClick={() => history.select(item)}
                  className={row}
                >
                  <span className="w-full truncate text-body text-fg">
                    {item.title ?? 'Sem título'}
                  </span>
                  <span className="w-full truncate text-small text-fg-subtle">
                    {conversationDetail(item)}
                  </span>
                </button>
              </li>
            ))}
          </ul>
        )}
      </div>
    </aside>
  );
}
