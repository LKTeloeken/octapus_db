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
import { useMainGroupWidth, usePixelPanel } from '@/lib/panel-sizes';
import { cn } from '@/lib/utils';
import { useCompactWindow } from '@/lib/window-size';
import { QueryProvider } from '@/providers/query-provider';
import { useSyncKeybindings } from '@/queries/use-keybindings';
import { useWebviewDiagnostics } from '@/queries/use-webview-diagnostics';
import { useAssistantStore } from '@/stores/assistant-store';
import { useColorMode, useUiStore } from '@/stores/ui-store';
import { StatusBar } from '@/features/status-bar/status-bar';
import { CustomToaster } from './components/Toaster';

/** Painéis laterais em px (convertidos em % da largura atual, ver panel-sizes) */
const SIDEBAR_MIN_PX = 200;
const SIDEBAR_DEFAULT_PX = 240;
const SIDEBAR_MAX_PERCENT = 50;
const ASSISTANT_MIN_PX = 300;
const ASSISTANT_DEFAULT_PX = 380;
const ASSISTANT_MAX_PERCENT = 55;

/** Os atalhos configurados (preferências) chegam ao store dos listeners */
const KeybindingsSync = () => {
  useSyncKeybindings();
  return null;
};

/** Estado da webview no log do app (rede, volta depois de muito tempo) */
const WebviewDiagnostics = () => {
  useWebviewDiagnostics();
  return null;
};

const App = () => {
  const theme = useUiStore(state => state.theme);
  const accent = useUiStore(state => state.accent);
  const density = useUiStore(state => state.density);
  const uiFont = useUiStore(state => state.uiFont);
  const monoFont = useUiStore(state => state.monoFont);
  const colorMode = useColorMode();
  const assistantOpen = useAssistantStore(state => state.panelOpen);
  // Em janela compacta não cabe o painel ao lado: o assistente é a janela toda
  const compact = useCompactWindow();
  const agentMode = useAssistantStore(
    state => state.panelOpen && (state.agentMode || compact),
  );
  const groupWidth = useMainGroupWidth();
  const sidebar = usePixelPanel(groupWidth, {
    defaultPx: SIDEBAR_DEFAULT_PX,
    minPx: SIDEBAR_MIN_PX,
    maxPercent: SIDEBAR_MAX_PERCENT,
  });
  const assistant = usePixelPanel(groupWidth, {
    defaultPx: ASSISTANT_DEFAULT_PX,
    minPx: ASSISTANT_MIN_PX,
    maxPercent: ASSISTANT_MAX_PERCENT,
  });

  // Tema, acento, densidade e fontes viram atributos no <html> (globals.css)
  useEffect(() => {
    const root = document.documentElement;
    root.classList.remove('light', 'dark');
    root.classList.add(colorMode);
    const set = (name: string, value: string | null) =>
      value === null
        ? root.removeAttribute(name)
        : root.setAttribute(name, value);
    set('data-theme', theme === 'fossa' || theme === 'recife' ? theme : null);
    set('data-accent', accent === 'tinta' ? null : accent);
    set('data-density', density === 'normal' ? null : density);
    set('data-ui-font', uiFont === 'geist' ? null : uiFont);
    set('data-mono-font', monoFont === 'geist' ? null : monoFont);
  }, [theme, colorMode, accent, density, uiFont, monoFont]);

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
          <div
            className={cn(
              'flex h-screen w-full flex-col bg-bg',
              agentMode && 'hidden',
            )}
          >
            <ResizablePanelGroup
              data-tauri-drag-region
              direction="horizontal"
              className="min-h-0 w-full flex-1 gap-1 bg-bg p-2 pb-0 text-fg"
            >
              <ResizablePanel
                id="sidebar"
                order={1}
                {...sidebar.panelProps}
                className="overflow-hidden rounded-lg border border-line bg-surface-1"
              >
                <ErrorBoundary area="na barra lateral">
                  <Sidebar />
                </ErrorBoundary>
              </ResizablePanel>

              <ResizableHandle
                className="cursor-col-resize! bg-transparent"
                onDragging={sidebar.onDragging}
              />

              <ResizablePanel id="workspace" order={2}>
                <QueryTabs />
              </ResizablePanel>

              {/* Assistente: painel próprio, com abas de conversa em memória
                (fora do tabs-store, que vai para o disco — RN-01). No modo
                agente o painel fica no grupo (escondido), vazio: tirá-lo
                junto com a mudança de largura que cruza a janela compacta
                quebrava o react-resizable-panels ("Panel data not found") */}
              {assistantOpen && (
                <>
                  <ResizableHandle
                    className="cursor-col-resize! bg-transparent"
                    onDragging={assistant.onDragging}
                  />
                  <ResizablePanel
                    id="assistant"
                    order={3}
                    {...assistant.panelProps}
                    className="overflow-hidden"
                  >
                    {!agentMode && (
                      <ErrorBoundary area="no assistente">
                        <AssistantPanel />
                      </ErrorBoundary>
                    )}
                  </ResizablePanel>
                </>
              )}
            </ResizablePanelGroup>
            {/* Barra de status: direto sobre o abismo, sem painel */}
            <StatusBar />
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

          <KeybindingsSync />
          <WebviewDiagnostics />
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
