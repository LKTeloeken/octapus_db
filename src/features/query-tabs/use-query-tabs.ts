import { useCallback, useEffect, useMemo } from 'react';
import { useActiveTab, useTabsStore } from '@/stores/tabs-store';
import { useQueryResultsStore } from '@/features/query-editor/query-results-store';

export const useQueryTabs = () => {
  const tabs = useTabsStore(state => state.tabs);
  const activeTabId = useTabsStore(state => state.activeTabId);
  const setActiveTab = useTabsStore(state => state.setActiveTab);
  const closeTabInStore = useTabsStore(state => state.closeTab);
  const clearRun = useQueryResultsStore(state => state.clearRun);

  const activeTab = useActiveTab();
  const tabsList = useMemo(() => Array.from(tabs.values()), [tabs]);

  const closeTab = useCallback(
    (id: string) => {
      closeTabInStore(id);
      clearRun(id); // drop the ephemeral editor result of the closed tab
    },
    [closeTabInStore, clearRun],
  );

  // Cmd/Ctrl+W closes the active tab; Cmd/Ctrl+1..9 jumps to the Nth tab
  useEffect(() => {
    const handleShortcut = (event: KeyboardEvent) => {
      if (!(event.metaKey || event.ctrlKey)) return;

      if (event.key.toLowerCase() === 'w') {
        event.preventDefault();
        if (activeTabId) closeTab(activeTabId);
        return;
      }

      if (/^[1-9]$/.test(event.key)) {
        const target = tabsList[Number(event.key) - 1];
        if (target) {
          event.preventDefault();
          setActiveTab(target.id);
        }
      }
    };

    window.addEventListener('keydown', handleShortcut);
    return () => window.removeEventListener('keydown', handleShortcut);
  }, [activeTabId, closeTab, tabsList, setActiveTab]);

  return {
    tabs: tabsList,
    activeTab,
    activeTabId,
    setActiveTab,
    closeTab,
  };
};
