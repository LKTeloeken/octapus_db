import { useEffect, type FC } from "react";
import {
  ResizablePanelGroup,
  ResizablePanel,
  ResizableHandle,
} from "@/components/ui/resizable";
import { CustomToaster } from "./components/Toaster";

import { ServersProvider } from "./shared/providers/servers-provider";
import { TabsProvider } from "./shared/providers/tabs-provider";

import Sidebar from "@/components/layout/sidebar";
import QueryContent from "@/components/layout/query-content/query-content";

const App: FC = () => {
  useEffect(() => {
    const root = document.documentElement;
    root.classList.remove("light", "dark");
    root.classList.add("dark");
  }, []);

  return (
    <>
      <CustomToaster />

      <ResizablePanelGroup direction="horizontal" className="h-screen w-full">
        <TabsProvider>
          <ResizablePanel
            defaultSize={20}
            minSize={15}
            maxSize={40}
            className="border-r border-border bg-sidebar text-sidebar-foreground"
          >
            <ServersProvider>
              <Sidebar />
            </ServersProvider>
          </ResizablePanel>

          <ResizableHandle withHandle />

          <QueryContent />
        </TabsProvider>
      </ResizablePanelGroup>
    </>
  );
};

export default App;
