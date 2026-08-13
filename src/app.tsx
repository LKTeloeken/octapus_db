import { useEffect } from 'react';
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from '@/components/ui/resizable';
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
      <ResizablePanelGroup direction="horizontal" className="h-screen w-full">
        <ResizablePanel
          defaultSize={20}
          minSize={20}
          maxSize={50}
          className="border-r border-border bg-sidebar text-sidebar-foreground"
        >
          <Sidebar />
        </ResizablePanel>

        <ResizableHandle className="cursor-col-resize!" />

        <ResizablePanel className="bg-main text-main-foreground p-2">
          <QueryTabs />
        </ResizablePanel>
      </ResizablePanelGroup>

      <CommandPalette />
      <CustomToaster />
      <UpdateNotifier />
    </QueryProvider>
  );
};

export default App;
