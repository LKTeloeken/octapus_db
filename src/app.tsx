import { ResultsTable } from '@/components/results-table/results-table';
import { QueryEditorContainer } from '@/components/query-editor/query-editor-container/query-editor-container';
import { GlobalSearchDialog } from '@/components/global-search-dialog/global-search-dialog';
import {
  ResizableHandle,
  ResizablePanel,
  ResizablePanelGroup,
} from '@/components/ui/resizable';
import { useDataStructure } from '@/shared/hooks/use-data-structure/use-data-structure';
import { useServers } from '@/shared/hooks/use-servers/use-servers';
import { useCallback, useEffect, useMemo } from 'react';
import { CustomToaster } from './components/Toaster';
import { QueryTabs } from './components/query-tabs/query-tabs';
import { SidebarBody } from './components/sidebar/sidebar-body/sidebar-body';
import useTabs from './shared/hooks/use-tabs/use-tabs';
import { TabType } from './shared/models/tabs.types';
import { useStore } from './stores';

const App = () => {
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
    openTableTab,
    closeTab,
    setActiveTabId,
    setTabContent,
    applyQueryTabChanges,
    runQueryTab,
    cancelQueryTab,
    sortTableTab,
    openTableByReference,
    handleNextPage,
    switchViewTabToQuery,
  } = useTabs(handleFetchStructure);
  const { viewLayout, setViewLayout, fetchColumns } = useStore();

  const handleOpenTableTab = useCallback(
    (nodeId: string) => {
      onClickNode(nodeId, openTableTab);
    },
    [onClickNode, openTableTab],
  );

  const currentStructure = useMemo(() => {
    if (activeTab) {
      return getStructure(activeTab.serverId, activeTab.databaseName);
    }

    return null;
  }, [activeTab, getStructure]);

  const handleRequestTableColumns = useCallback(
    async (schemaName: string, tableName: string) => {
      if (!activeTab) return;
      await fetchColumns(
        activeTab.serverId,
        activeTab.databaseName,
        schemaName,
        tableName,
      );
    },
    [activeTab, fetchColumns],
  );

  // Run the query for the view tab when it is active
  useEffect(() => {
    if (activeTab && activeTab?.type === TabType.View) {
      runQueryTab(activeTab.id, activeTab.content);
    }
  }, [activeTab?.id]);

  useEffect(() => {
    const root = document.documentElement;
    root.classList.remove('light', 'dark');
    root.classList.add('dark');

    fetchServers();
  }, []);

  useEffect(() => {
    const handleCloseTabShortcut = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey)) return;
      if (event.key.toLowerCase() !== 'w') return;
      if (!activeTab || activeTab.type !== TabType.View) return;

      event.preventDefault();
      closeTab(activeTab.id);
    };

    window.addEventListener('keydown', handleCloseTabShortcut);
    return () => window.removeEventListener('keydown', handleCloseTabShortcut);
  }, [activeTab, closeTab]);

  return (
    <>
      <ResizablePanelGroup direction="horizontal" className="h-screen w-full">
        <ResizablePanel
          defaultSize={20}
          minSize={20}
          maxSize={50}
          className="border-r border-border bg-sidebar text-sidebar-foreground"
        >
          <SidebarBody
            nodes={nodes.nodes}
            childrenMap={nodes.childrenMap}
            isLoading={isLoading}
            toggleNode={handleOpenTableTab}
            onCreateServer={addServer}
            onEditServer={editServer}
            onDeleteServer={removeServer}
            openTab={openTab}
          />
        </ResizablePanel>
        <ResizableHandle className="cursor-col-resize!" />
        <ResizablePanel className="bg-main text-main-foreground p-2">
          {tabs.length > 0 && (
            <QueryTabs
              tabs={tabs}
              activeTabId={activeTab?.id || ''}
              onTabChange={setActiveTabId}
              onTabClose={closeTab}
            >
              {activeTab &&
                (activeTab.type === TabType.Query ? (
                  <ResizablePanelGroup direction="vertical" className="h-full">
                    <ResizablePanel defaultSize={40} minSize={20}>
                      <QueryEditorContainer
                        value={activeTab.content}
                        onChange={content =>
                          setTabContent(activeTab.id, content)
                        }
                        onExecute={query => runQueryTab(activeTab.id, query)}
                        onCancel={() => cancelQueryTab(activeTab.id)}
                        isQueryRunning={activeTab?.loading || false}
                        isLoading={activeTab?.loading || false}
                        databaseStructure={currentStructure}
                        onRequestTableColumns={handleRequestTableColumns}
                      />
                    </ResizablePanel>
                    <ResizableHandle className="bg-transparent my-1 cursor-col-resize!" />
                    <ResizablePanel defaultSize={60} minSize={20}>
                      <ResultsTable
                        columns={activeTab?.result?.columns || []}
                        rows={activeTab?.result?.rows || []}
                        editableInfo={activeTab?.result?.editableInfo}
                        isLoading={activeTab?.loading || false}
                        isLoadingMore={activeTab?.loadingMore || false}
                        hasMore={activeTab?.result?.hasMore || false}
                        executionTimeMs={activeTab?.result?.executionTimeMs}
                        totalCount={activeTab?.result?.totalCount}
                        rowCount={activeTab?.result?.rowCount}
                        onLoadMore={() =>
                          activeTab && handleNextPage(activeTab.id)
                        }
                        onApplyChanges={edits =>
                          applyQueryTabChanges(activeTab.id, edits)
                        }
                        tabType={activeTab.type}
                        sort={activeTab.sort}
                        onSortColumn={column =>
                          sortTableTab(activeTab.id, column)
                        }
                        onOpenForeignTable={target =>
                          openTableByReference(
                            activeTab.serverId,
                            activeTab.databaseName,
                            target.schema,
                            target.table,
                          )
                        }
                        viewLayout={viewLayout}
                        onViewLayoutChange={setViewLayout}
                        className="h-full"
                      />
                    </ResizablePanel>
                  </ResizablePanelGroup>
                ) : (
                  <ResultsTable
                    columns={activeTab?.result?.columns || []}
                    rows={activeTab?.result?.rows || []}
                    editableInfo={activeTab?.result?.editableInfo}
                    isLoading={activeTab?.loading || false}
                    isLoadingMore={activeTab?.loadingMore || false}
                    hasMore={activeTab?.result?.hasMore || false}
                    executionTimeMs={activeTab?.result?.executionTimeMs}
                    totalCount={activeTab?.result?.totalCount}
                    rowCount={activeTab?.result?.rowCount}
                    onLoadMore={() => activeTab && handleNextPage(activeTab.id)}
                    onApplyChanges={edits =>
                      applyQueryTabChanges(activeTab.id, edits)
                    }
                    tabType={activeTab.type}
                    sort={activeTab.sort}
                    onSortColumn={column => sortTableTab(activeTab.id, column)}
                    onOpenForeignTable={target =>
                      openTableByReference(
                        activeTab.serverId,
                        activeTab.databaseName,
                        target.schema,
                        target.table,
                      )
                    }
                    viewLayout={viewLayout}
                    onViewLayoutChange={setViewLayout}
                    onSwitchToSql={() => switchViewTabToQuery(activeTab.id)}
                    className="h-full"
                  />
                ))}
            </QueryTabs>
          )}
        </ResizablePanel>
      </ResizablePanelGroup>

      <GlobalSearchDialog
        nodes={nodes.nodes}
        onOpenTable={openTableByReference}
      />

      <CustomToaster />
    </>
  );
};

export default App;
