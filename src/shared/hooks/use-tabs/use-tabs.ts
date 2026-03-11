import { type Tab, TabType } from '@/shared/models/tabs.types';
import { useMemo, useState } from 'react';
import type { HandleFetchStructure } from '../use-data-structure/use-data-structure.types';
import type {
  CloseTab,
  OpenTab,
  SetActiveTabId,
  SetTabContent,
} from './use-tabs.types';
import { useBuildQueries } from '../use-build-queries/use-build-queries';
import { TreeNodeType, type TreeNode } from '@/shared/models/database.types';
import { useQueryTabs } from '../use-query-tabs/use-query-tabs';

const useTabs = (loadDatabaseStructure: HandleFetchStructure) => {
  const [tabs, setTabs] = useState<Tab[]>([]);
  const {
    queryTabs,
    addQueryTab,
    closeQueryTab,
    applyQueryTabChanges,
    runQueryTab,
    handleNextPage,
  } = useQueryTabs();
  const { selectQuery } = useBuildQueries();

  const activeTab = useMemo(() => tabs.find(t => t.active), [tabs]);

  const handleSetTab = (id: string, content: Partial<Tab>) => {
    setTabs(prevTabs =>
      prevTabs.map(tab => (tab.id === id ? { ...tab, ...content } : tab)),
    );
  };

  const openTab: OpenTab = (serverId, databaseName, initalData) => {
    const newTabId = `${serverId}-${databaseName}-${Date.now()}`;

    setTabs(prevTabs =>
      [
        ...prevTabs,
        {
          id: newTabId,
          serverId,
          databaseName,
          title: databaseName,
          content: '',
          active: true,
          type: TabType.View,
          ...initalData,
        },
      ].map(tab => ({
        ...tab,
        active: tab.id === newTabId ? true : false,
      })),
    );

    if (initalData?.active) {
      setActiveTabId(newTabId);
    }

    loadDatabaseStructure(serverId, databaseName);
    addQueryTab(newTabId, serverId, databaseName);

    // If the tab is opened with a query, execute it
    if (initalData?.content) {
      runQueryTab(newTabId, initalData.content, undefined, {
        serverId,
        databaseName,
      });
    }
  };

  const openTableTab = (node: TreeNode) => {
    if (node.type !== TreeNodeType.Table) return;

    const [, tableName, schemaName, databaseName, serverId] =
      node.id.split('-');
    const tableQuery = selectQuery(schemaName, tableName);

    openTab(Number(serverId), databaseName, {
      title: tableName,
      content: tableQuery,
      active: true,
      type: TabType.View,
    });
  };

  const closeTab: CloseTab = id => {
    setTabs(prevTabs => prevTabs.filter(tab => tab.id !== id));
    closeQueryTab(id);
  };

  const setActiveTabId: SetActiveTabId = id => {
    setTabs(prevTabs =>
      prevTabs.map(tab => ({ ...tab, active: tab.id === id ? true : false })),
    );
  };

  const setTabContent: SetTabContent = (id, content) => {
    handleSetTab(id, { content });
  };

  return {
    tabs,
    activeTab,
    queryTabs,
    openTab,
    addQueryTab,
    closeQueryTab,
    applyQueryTabChanges,
    runQueryTab,
    handleNextPage,
    closeTab,
    setActiveTabId,
    setTabContent,
    openTableTab,
  };
};

export default useTabs;
