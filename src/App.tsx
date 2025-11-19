import { useEffect, type FC } from "react";
import {
  ResizablePanelGroup,
  ResizablePanel,
  ResizableHandle,
} from "@/components/ui/resizable";
import { CustomToaster } from "./components/Toaster";
import { SidebarBody } from "./components/sidebar/sidebar-body/sidebar-body";
import { useServers } from "@/shared/hooks/use-servers/use-servers";
import { useDataStructure } from "@/shared/hooks/use-data-structure/use-data-structure";

const App: FC = () => {
  const { nodes, toggleNode, addChildrenToState, removeChildrenFromState } =
    useDataStructure();
  const { addServer, editServer, fetchServers, isLoading } = useServers({
    addChildren: addChildrenToState,
  });

  useEffect(() => {
    fetchServers();
  }, []);

  useEffect(() => {
    const root = document.documentElement;
    root.classList.remove("light", "dark");
    root.classList.add("dark");
  }, []);

  return (
    <>
      <CustomToaster />

      <ResizablePanelGroup direction="horizontal" className="h-screen w-full">
        <ResizablePanel
          defaultSize={20}
          minSize={15}
          maxSize={50}
          className="border-r border-border bg-sidebar text-sidebar-foreground"
        >
          <SidebarBody
            nodes={nodes.nodes}
            childrenMap={nodes.childrenMap}
            isLoading={isLoading}
            toggleNode={toggleNode}
            onCreateServer={() => {}}
            onEditServer={() => {}}
            onDeleteServer={() => {}}
          />
        </ResizablePanel>
        <ResizableHandle />
        <ResizablePanel className="bg-main text-main-foreground">
          {/* Main content goes here */}
        </ResizablePanel>
      </ResizablePanelGroup>
    </>
  );
};

export default App;
