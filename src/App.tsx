import { useEffect, type FC } from "react";
import {
  ResizablePanelGroup,
  ResizablePanel,
  ResizableHandle,
} from "@/components/ui/resizable";
import { CustomToaster } from "./components/Toaster";
import { SidebarBody } from "./components/sidebar/sidebar-body/sidebar-body";
import { QueryTabs } from "./components/query-tabs/query-tabs";
import { useServers } from "@/shared/hooks/use-servers/use-servers";
import { useDataStructure } from "@/shared/hooks/use-data-structure/use-data-structure";
import { useQueryTabs } from "./shared/hooks/use-query-tabs/use-query-tabs";

const App: FC = () => {
  const { nodes, toggleNode, addChildrenToState, removeNode } =
    useDataStructure();
  const { addServer, removeServer, editServer, fetchServers, isLoading } =
    useServers({
      addChildren: addChildrenToState,
      removeNode,
    });
  const { tabs, activeTab, openTab, closeTab, setActiveTabId } = useQueryTabs();

  useEffect(() => {
    const root = document.documentElement;
    root.classList.remove("light", "dark");
    root.classList.add("dark");

    fetchServers();
  }, []);

  return (
    <>
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
            onCreateServer={addServer}
            onEditServer={editServer}
            onDeleteServer={removeServer}
            openTab={openTab}
          />
        </ResizablePanel>
        <ResizableHandle />
        <ResizablePanel className="bg-main text-main-foreground">
          {activeTab && (
            <QueryTabs
              tabs={tabs}
              activeTabId={activeTab.id}
              onTabChange={setActiveTabId}
              onTabClose={closeTab}
            />
          )}
        </ResizablePanel>
      </ResizablePanelGroup>

      <CustomToaster />
    </>
  );
};

export default App;
