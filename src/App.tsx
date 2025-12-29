import { useEffect, useMemo, type FC } from "react";
import {
  ResizablePanelGroup,
  ResizablePanel,
  ResizableHandle,
} from "@/components/ui/resizable";
import { CustomToaster } from "./components/Toaster";
import { SidebarBody } from "./components/sidebar/sidebar-body/sidebar-body";
import { QueryTabs } from "./components/query-tabs/query-tabs";
import { QueryEditorContainer } from "@/components/query-editor/query-editor-container/query-editor-container";
import { ResultsContainer } from "@/components/query-results/results-container/results-container";
import { useServers } from "@/shared/hooks/use-servers/use-servers";
import { useDataStructure } from "@/shared/hooks/use-data-structure/use-data-structure";
import { useQueryTabs } from "./shared/hooks/use-query-tabs/use-query-tabs";

const App: FC = () => {
  const {
    nodes,
    onClickNode,
    addNodes,
    removeNode,
    handleFetchStructure,
    getStructure,
  } = useDataStructure();
  const { addServer, removeServer, editServer, fetchServers, isLoading } =
    useServers({
      addChildren: addNodes,
      removeNode,
    });
  const {
    tabs,
    activeTab,
    openTab,
    closeTab,
    setActiveTabId,
    setTabContent,
    setTabQuery,
    executeQuery,
  } = useQueryTabs(handleFetchStructure);

  const currentStructure = useMemo(() => {
    if (activeTab) {
      return getStructure(activeTab.serverId, activeTab.databaseName);
    }

    return null;
  }, [tabs, getStructure]);

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
            toggleNode={onClickNode}
            onCreateServer={addServer}
            onEditServer={editServer}
            onDeleteServer={removeServer}
            openTab={openTab}
          />
        </ResizablePanel>
        <ResizableHandle />
        <ResizablePanel className="bg-main text-main-foreground p-2">
          {tabs.length > 0 && (
            <QueryTabs
              tabs={tabs}
              activeTabId={activeTab?.id}
              onTabChange={setActiveTabId}
              onTabClose={closeTab}
            >
              {activeTab && (
                <ResizablePanelGroup direction="vertical" className="h-full">
                  <ResizablePanel defaultSize={40} minSize={20}>
                    <QueryEditorContainer
                      value={activeTab.content}
                      onChange={(content) =>
                        setTabContent(activeTab.id, content)
                      }
                      onChangeSelection={(selection) =>
                        setTabQuery(activeTab.id, selection)
                      }
                      onExecute={() =>
                        executeQuery(
                          activeTab.id,
                          activeTab.query || activeTab.content
                        )
                      }
                      isLoading={activeTab.loading}
                      databaseStructure={currentStructure}
                    />
                  </ResizablePanel>
                  <ResizableHandle className="bg-transparent my-1" />
                  <ResizablePanel defaultSize={60} minSize={20}>
                    <ResultsContainer
                      columns={activeTab.result?.fields || []}
                      rows={activeTab.result?.rows || []}
                      isLoading={activeTab.loading}
                      className="h-full"
                    />
                  </ResizablePanel>
                </ResizablePanelGroup>
              )}
            </QueryTabs>
          )}
        </ResizablePanel>
      </ResizablePanelGroup>

      <CustomToaster />
    </>
  );
};

export default App;
