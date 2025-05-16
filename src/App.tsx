import React, { useEffect } from "react";
import {
  ResizablePanelGroup,
  ResizablePanel,
  ResizableHandle,
} from "@/components/ui/resizable";

import { ServersProvider } from "./providers/serversProvider";

import Sidebar from "@/components/layout/Sidebar/index";
import QueryEditor from "@/components/layout/QueryEditor";
import QueryResults from "@/components/layout/QueryResults";

const App: React.FC = () => {
  // força tema dark (igual ao seu exemplo)
  useEffect(() => {
    const root = document.documentElement;
    root.classList.remove("light", "dark");
    root.classList.add("dark");
  }, []);

  return (
    <ResizablePanelGroup direction="horizontal" className="h-screen w-full">
      <ResizablePanel
        defaultSize={20} // 20 % da largura
        minSize={10}
        maxSize={40}
        className="border-r border-border bg-sidebar text-sidebar-foreground"
      >
        <ServersProvider>
          <Sidebar />
        </ServersProvider>
      </ResizablePanel>

      <ResizableHandle withHandle />

      <ResizablePanel defaultSize={80} className="overflow-hidden">
        <ResizablePanelGroup direction="vertical" className="h-full w-full">
          <ResizablePanel
            defaultSize={40}
            minSize={20}
            maxSize={80}
            className="border-b border-border"
          >
            <QueryEditor />
          </ResizablePanel>

          <ResizableHandle withHandle />

          <ResizablePanel defaultSize={60} minSize={20}>
            <QueryResults />
          </ResizablePanel>
        </ResizablePanelGroup>
      </ResizablePanel>
    </ResizablePanelGroup>
  );
};

export default App;
