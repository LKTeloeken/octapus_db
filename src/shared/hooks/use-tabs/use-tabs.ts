import type { Tab } from '@/shared/models/tabs.types';
import { useMemo, useState } from 'react';
import type { HandleFetchStructure } from '../use-data-structure/use-data-structure.types';
import type {
  CloseTab,
  OpenTab,
  SetActiveTabId,
  SetTabContent,
} from './use-tabs.types';

const useTabs = (loadDatabaseStructure: HandleFetchStructure) => {
  const [tabs, setTabs] = useState<Tab[]>([]);

  const activeTab = useMemo(() => tabs.find(t => t.active), [tabs]);

  const handleSetTab = (id: string, content: Partial<Tab>) => {
    setTabs(prevTabs =>
      prevTabs.map(tab => (tab.id === id ? { ...tab, ...content } : tab)),
    );
  };

  const openTab: OpenTab = (serverId, databaseName) => {
    const newTabId = `${serverId}-${databaseName}-${Date.now()}`;

    setTabs(prevTabs => [
      ...prevTabs,
      {
        id: newTabId,
        serverId,
        databaseName,
        title: databaseName,
        content: '',
        active: true,
      },
    ]);

    loadDatabaseStructure(serverId, databaseName);
  };

  const closeTab: CloseTab = id => {
    handleSetTab(id, { active: false });
  };

  const setActiveTabId: SetActiveTabId = id => {
    handleSetTab(id, { active: true });
  };

  const setTabContent: SetTabContent = (id, content) => {
    handleSetTab(id, { content });
  };

  return {
    tabs,
    activeTab,
    openTab,
    closeTab,
    setActiveTabId,
    setTabContent,
  };
};

export default useTabs;
