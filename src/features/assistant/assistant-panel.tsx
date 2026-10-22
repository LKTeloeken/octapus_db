import {
  Add01Icon,
  Alert02Icon,
  ArrowExpand01Icon,
  ArrowShrink01Icon,
  Cancel01Icon,
  CloudOffIcon,
  Download04Icon,
  PlugSocketIcon,
} from '@hugeicons/core-free-icons';
import { HugeiconsIcon } from '@hugeicons/react';
import { Button } from '@/components/ui/button';
import { ContentState } from '@/components/ui/content-state';
import { Kbd } from '@/components/ui/kbd';
import { Separator } from '@/components/ui/separator';
import { Tabs, TabsList, TabsTrigger } from '@/components/ui/tabs';
import { Tooltip } from '@/components/ui/tooltip/tooltip';
import { OctapusAnimated } from '@/components/octapus-animated/octapus-animated';
import { OctapusThinking } from '@/components/octapus-thinking/octapus-thinking';
import {
  TrafficLights,
  WindowControls,
} from '@/features/window-controls/window-controls';
import { customTitlebar, isMac, shortcut } from '@/lib/platform';
import { cn } from '@/lib/utils';
import { AgentSidebar } from './agent-sidebar';
import { ConsentView } from './consent-view';
import { ConversationView } from './conversation-view';
import { HistoryMenu } from './history-menu';
import { LoginForm } from './login-form';
import { useAssistantPanel } from './use-assistant-panel';
import type { AssistantGate } from './assistant-gate';

/** `Cmd/Ctrl+Shift+J` no rótulo do botão */
const AGENT_MODE_KEY = shortcut(isMac ? '⇧J' : 'Shift+J');

/**
 * O painel do assistente, à direita das abas. Tem as próprias abas de
 * conversa (em memória — RN-01) e a sequência de portas antes de perguntar:
 * configurado → no ar → login → termo.
 *
 * No modo agente ele é a janela inteira: as conversas vão para uma coluna à
 * esquerda, a conversa ativa fica centrada e o cabeçalho vira a barra de
 * título (semáforo no macOS, controles no Linux).
 */
export function AssistantPanel() {
  const {
    gate,
    tabs,
    activeTab,
    activeTabId,
    setActiveTab,
    closeTab,
    newTab,
    closePanel,
    agentMode,
    toggleAgentMode,
    retry,
    isRetrying,
  } = useAssistantPanel();
  const ready = gate.kind === 'ready';
  const activeTitle = tabs.find(tab => tab.id === activeTabId)?.title;

  return (
    <section
      aria-label="Assistente"
      data-assistant-panel=""
      data-keyboard-scope="assistant"
      className={cn(
        'flex h-full min-h-0 flex-col overflow-hidden rounded-lg border border-line bg-surface-1',
        agentMode && 'animate-in fade-in-0 duration-240 ease-out',
      )}
    >
      <header
        data-tauri-drag-region="deep"
        className="flex h-11 shrink-0 items-center gap-1 border-b border-line-subtle px-1.5"
      >
        {agentMode && customTitlebar === 'mac' && (
          <div className="pr-2 pl-2">
            <TrafficLights />
          </div>
        )}
        {agentMode ? (
          <span className="min-w-0 truncate px-2 text-heading font-semibold text-fg">
            {ready && activeTitle ? activeTitle : 'Assistente'}
          </span>
        ) : ready && tabs.length > 0 ? (
          <Tabs
            value={activeTabId ?? ''}
            onValueChange={setActiveTab}
            className="min-w-0 shrink"
          >
            <TabsList
              variant="document"
              className="min-w-0 justify-start overflow-x-auto overflow-y-hidden no-scrollbar px-0.5"
            >
              {tabs.map(tab => (
                <div key={tab.id} className="group relative shrink-0">
                  <TabsTrigger
                    value={tab.id}
                    className="gap-1.5 pr-7"
                    onMouseDown={event => {
                      if (event.button === 1) event.preventDefault();
                    }}
                    onAuxClick={event => {
                      if (event.button === 1) closeTab(tab.id);
                    }}
                  >
                    {tab.running && <OctapusThinking className="size-3.5" />}
                    <span className="max-w-36 truncate">{tab.title}</span>
                  </TabsTrigger>
                  <button
                    type="button"
                    aria-label={`Fechar conversa ${tab.title}`}
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
              ))}
            </TabsList>
          </Tabs>
        ) : (
          <span className="px-2 text-heading font-semibold text-fg">
            Assistente
          </span>
        )}

        {ready && !agentMode && (
          <Tooltip
            content={`Nova conversa (${shortcut('T')})`}
            position="bottom"
          >
            <Button
              variant="ghost"
              size="icon"
              aria-label="Nova conversa"
              onClick={newTab}
            >
              <HugeiconsIcon icon={Add01Icon} />
            </Button>
          </Tooltip>
        )}

        <span className="flex-1" />

        {ready && !agentMode && <HistoryMenu />}

        <Tooltip
          content={
            agentMode
              ? `Sair do modo agente (${AGENT_MODE_KEY})`
              : `Modo agente (${AGENT_MODE_KEY})`
          }
          position="bottom"
        >
          <Button
            variant="ghost"
            size="icon"
            aria-label={agentMode ? 'Sair do modo agente' : 'Modo agente'}
            aria-pressed={agentMode}
            onClick={toggleAgentMode}
          >
            <HugeiconsIcon
              icon={agentMode ? ArrowShrink01Icon : ArrowExpand01Icon}
            />
          </Button>
        </Tooltip>

        <Tooltip content={`Fechar (${shortcut('J')})`} position="bottom">
          <Button
            variant="ghost"
            size="icon"
            aria-label="Fechar o assistente"
            onClick={closePanel}
          >
            <HugeiconsIcon icon={Cancel01Icon} />
          </Button>
        </Tooltip>

        {agentMode && customTitlebar === 'linux' && (
          <>
            <Separator
              orientation="vertical"
              className="mx-1 h-[18px]! bg-line"
            />
            <WindowControls />
          </>
        )}
      </header>

      <div className="flex min-h-0 flex-1">
        {agentMode && ready && (
          <AgentSidebar
            tabs={tabs}
            activeTabId={activeTabId}
            onSelect={setActiveTab}
            onClose={closeTab}
            onNew={newTab}
          />
        )}
        <div className="min-h-0 min-w-0 flex-1">
          {ready ? (
            activeTab ? (
              <ConversationView
                key={activeTab.id}
                tab={activeTab}
                me={gate.me}
                wide={agentMode}
              />
            ) : (
              <ContentState
                icon={Add01Icon}
                title="Nenhuma conversa aberta"
                description="Comece uma conversa nova ou reabra uma do histórico."
                action={
                  <Button variant="outline" size="sm" onClick={newTab}>
                    Nova conversa
                    <Kbd>{shortcut('T')}</Kbd>
                  </Button>
                }
              />
            )
          ) : (
            <GateState gate={gate} onRetry={retry} isRetrying={isRetrying} />
          )}
        </div>
      </div>
    </section>
  );
}

function GateState({
  gate,
  onRetry,
  isRetrying,
}: {
  gate: Exclude<AssistantGate, { kind: 'ready' }>;
  onRetry: () => void;
  isRetrying: boolean;
}) {
  const retryButton = (
    <Button variant="outline" size="sm" onClick={onRetry} disabled={isRetrying}>
      {isRetrying ? 'Tentando…' : 'Tentar de novo'}
    </Button>
  );

  switch (gate.kind) {
    case 'loading':
      return (
        <div className="flex h-full items-center justify-center">
          <OctapusAnimated className="size-12" />
        </div>
      );
    case 'not-configured':
      return (
        <ContentState
          icon={PlugSocketIcon}
          title="Assistente indisponível nesta versão"
          description="Este build do Octapus não tem o endereço do serviço do assistente. O resto do app funciona normalmente."
        />
      );
    case 'update-required':
      return (
        <ContentState
          icon={Download04Icon}
          title="Atualize o Octapus"
          description={
            gate.minAppVersion
              ? `O assistente precisa da versão ${gate.minAppVersion} ou mais nova.`
              : 'O assistente precisa de uma versão mais nova do app.'
          }
        />
      );
    case 'offline':
      return (
        <ContentState
          icon={CloudOffIcon}
          tone="danger"
          title="Sem conexão com o assistente"
          description="O serviço do assistente não respondeu. O resto do Octapus continua funcionando normalmente."
          action={retryButton}
        />
      );
    case 'login':
      return <LoginForm />;
    case 'no-term':
      return (
        <ContentState
          icon={Alert02Icon}
          title="Assistente ainda não liberado"
          description="O termo de consentimento ainda não foi publicado. Tente mais tarde."
          action={retryButton}
        />
      );
    case 'consent':
      return <ConsentView />;
    case 'error':
      return (
        <ContentState
          icon={Alert02Icon}
          tone="danger"
          title="O assistente respondeu com erro"
          description={gate.message}
          action={retryButton}
        />
      );
  }
}
