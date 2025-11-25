import { useCallback, useMemo, useState } from "react";
import { useRunQuery } from "@/shared/hooks/use-run-query/use-run-query";

import type {
  OpenTab,
  CloseTab,
  SetActiveTabId,
  SetTabContent,
  ExecuteQuery,
} from "@/shared/hooks/use-query-tabs/use-query-tabs.types";
import type { QueryTab } from "@/shared/models/query-tabs.types";

export function useQueryTabs() {
  const { runQuery, loading: loadingQuery } = useRunQuery();
  const [tabs, setTabs] = useState<QueryTab[]>([]);

  const activeTab = useMemo(() => tabs.find((t) => t.active), [tabs]);

  const openTab: OpenTab = useCallback(
    (serverId: number, databaseName: string) => {
      const newTabId = `${serverId}-${databaseName}-${Date.now()}`;

      setTabs((prevTabs) => [
        ...prevTabs.map((tab) => ({ ...tab, active: false })),
        {
          id: newTabId,
          serverId,
          databaseName,
          title: databaseName,
          content: "",
          active: true,
        },
      ]);
    },
    []
  );

  const closeTab: CloseTab = useCallback((id: string) => {
    setTabs((prevTabs) => prevTabs.filter((tab) => tab.id !== id));
  }, []);

  const setActiveTabId: SetActiveTabId = useCallback((id: string) => {
    setTabs((prevTabs) =>
      prevTabs.map((tab) => ({
        ...tab,
        active: tab.id === id,
      }))
    );
  }, []);

  const setTabContent: SetTabContent = useCallback(
    (id: string, content: string) => {
      setTabs((prevTabs) =>
        prevTabs.map((tab) => (tab.id === id ? { ...tab, content } : tab))
      );
    },
    []
  );

  const executeQuery: ExecuteQuery = useCallback(
    async (id: string, query: string) => {
      const tab = tabs.find((t) => t.id === id);
      if (!tab) return;

      setTabs((prevTabs) =>
        prevTabs.map((t) => (t.id === id ? { ...t, loading: true } : t))
      );

      const { rows, fields } = await runQuery(
        tab.serverId,
        tab.databaseName,
        query
      );

      setTabs((prevTabs) =>
        prevTabs.map((t) =>
          t.id === id ? { ...t, result: { rows, fields }, loading: false } : t
        )
      );
    },
    [tabs, runQuery]
  );

  return {
    tabs,
    activeTab,
    loadingQuery,
    openTab,
    closeTab,
    setActiveTabId,
    setTabContent,
    executeQuery,
  };
}
