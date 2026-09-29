import { useEffect } from 'react';
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from '@/components/ui/resizable';
import { TooltipProvider } from '@/components/ui/tooltip/tooltip';
import { CommandPalette } from '@/features/command-palette/command-palette';
import { QueryTabs } from '@/features/query-tabs/query-tabs';
import { Sidebar } from '@/features/sidebar/sidebar';
import { UpdateNotifier } from '@/features/update-notifier/update-notifier';
import { QueryProvider } from '@/providers/query-provider';
import { useUiStore } from '@/stores/ui-store';
import { CustomToaster } from './components/Toaster';

const App = () => {
  const theme = useUiStore(state => state.theme);

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
        <ResizablePanelGroup
          direction="horizontal"
          className="h-screen w-full gap-1 bg-bg p-2 text-fg"
        >
          <ResizablePanel
            defaultSize={20}
            minSize={20}
            maxSize={50}
            className="overflow-hidden rounded-lg border border-line bg-surface-1"
          >
            <Sidebar />
          </ResizablePanel>

          <ResizableHandle className="cursor-col-resize! bg-transparent" />

          <ResizablePanel>
            <QueryTabs />
          </ResizablePanel>
        </ResizablePanelGroup>

        <CommandPalette />
        <CustomToaster />
        <UpdateNotifier />
      </TooltipProvider>
    </QueryProvider>
  );
};

export default App;
