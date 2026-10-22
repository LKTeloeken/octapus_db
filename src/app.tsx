import { useEffect } from 'react';
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from '@/components/ui/resizable';
import { ErrorBoundary } from '@/components/error-boundary';
import { TooltipProvider } from '@/components/ui/tooltip/tooltip';
import { AssistantPanel } from '@/features/assistant/assistant-panel';
import { AssistantShortcutsHost } from '@/features/assistant/assistant-shortcuts-host';
import { BackupDialogs } from '@/features/backup/backup-dialogs';
import { BackupJobs } from '@/features/backup/backup-jobs';
import { CommandPalette } from '@/features/command-palette/command-palette';
import { QueryTabs } from '@/features/query-tabs/query-tabs';
import { Sidebar } from '@/features/sidebar/sidebar';
import { UpdateNotifier } from '@/features/update-notifier/update-notifier';
import { cn } from '@/lib/utils';
import { QueryProvider } from '@/providers/query-provider';
import { useAssistantStore } from '@/stores/assistant-store';
import { useUiStore } from '@/stores/ui-store';
import { CustomToaster } from './components/Toaster';

const App = () => {
  const theme = useUiStore(state => state.theme);
  const assistantOpen = useAssistantStore(state => state.panelOpen);
  const agentMode = useAssistantStore(
    state => state.panelOpen && state.agentMode,
  );

  useEffect(() => {
    const root = document.documentElement;
    root.classList.remove('light', 'dark');
    root.classList.add(theme);
  }, [theme]);

  return (
    <QueryProvider>
      {/* 500 ms antes do primeiro tooltip; entre vizinhos (skipDelayDuration)
          eles abrem na hora, sem piscar ao atravessar a toolbar. */}
      <TooltipProvider delayDuration={500}>
        {/* Último limite: o que escapar dos de baixo não deixa a janela vazia */}
        <ErrorBoundary area="no app" offerReload className="h-screen bg-bg">
          {/* A moldura de 8 px em volta dos painéis também arrasta a janela.
              No modo agente o grupo só se esconde: a árvore e as abas de
              query continuam montadas (editor, resultados, scroll). */}
          {/* O `hidden` vai num invólucro: o grupo põe `display: flex` inline */}
          <div className={cn('h-screen w-full', agentMode && 'hidden')}>
            <ResizablePanelGroup
              data-tauri-drag-region
              direction="horizontal"
              className="h-screen w-full gap-1 bg-bg p-2 text-fg"
            >
              <ResizablePanel
                id="sidebar"
                order={1}
                defaultSize={20}
                minSize={20}
                maxSize={50}
                className="overflow-hidden rounded-lg border border-line bg-surface-1"
              >
                <ErrorBoundary area="na barra lateral">
                  <Sidebar />
                </ErrorBoundary>
              </ResizablePanel>

              <ResizableHandle className="cursor-col-resize! bg-transparent" />

              <ResizablePanel id="workspace" order={2}>
                <QueryTabs />
              </ResizablePanel>

              {/* Assistente: painel próprio, com abas de conversa em memória
                (fora do tabs-store, que vai para o disco — RN-01) */}
              {assistantOpen && !agentMode && (
                <>
                  <ResizableHandle className="cursor-col-resize! bg-transparent" />
                  <ResizablePanel
                    id="assistant"
                    order={3}
                    defaultSize={30}
                    minSize={22}
                    maxSize={55}
                    className="overflow-hidden"
                  >
                    <ErrorBoundary area="no assistente">
                      <AssistantPanel />
                    </ErrorBoundary>
                  </ResizablePanel>
                </>
              )}
            </ResizablePanelGroup>
          </div>

          {/* Modo agente: o assistente é a janela inteira */}
          {agentMode && (
            <div
              data-tauri-drag-region
              className="h-screen w-full bg-bg p-2 text-fg"
            >
              <ErrorBoundary area="no assistente">
                <AssistantPanel />
              </ErrorBoundary>
            </div>
          )}

          <CommandPalette />
          <AssistantShortcutsHost />
          <BackupDialogs />
          <CustomToaster />
          <BackupJobs />
          <UpdateNotifier />
        </ErrorBoundary>
      </TooltipProvider>
    </QueryProvider>
  );
};

export default App;
