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
import toast from "react-hot-toast";
import type { HandleFetchStructure } from "../use-data-structure/use-data-structure.types";

export function useQueryTabs(loadDatabaseStructure: HandleFetchStructure) {
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

      loadDatabaseStructure(serverId, databaseName).catch((error) => {
        toast.error(
          `Failed to load database structure for ${databaseName}: ${String(
            error,
          )}`,
        );
      });
    },
    [],
  );

  const closeTab: CloseTab = useCallback((id: string) => {
    setTabs((prevTabs) => prevTabs.filter((tab) => tab.id !== id));
  }, []);

  const setActiveTabId: SetActiveTabId = useCallback((id: string) => {
    setTabs((prevTabs) =>
      prevTabs.map((tab) => ({
        ...tab,
        active: tab.id === id,
      })),
    );
  }, []);

  const setTabContent: SetTabContent = useCallback(
    (id: string, content: string) => {
      setTabs((prevTabs) =>
        prevTabs.map((tab) => (tab.id === id ? { ...tab, content } : tab)),
      );
    },
    [],
  );

  const setTabQuery: SetTabContent = useCallback(
    (id: string, query: string) => {
      setTabs((prevTabs) =>
        prevTabs.map((tab) => (tab.id === id ? { ...tab, query } : tab)),
      );
    },
    [],
  );

  const executeQuery: ExecuteQuery = useCallback(
    async (id: string, query: string) => {
      const tab = tabs.find((t) => t.id === id);
      if (!tab) return;

      setTabs((prevTabs) =>
        prevTabs.map((t) => (t.id === id ? { ...t, loading: true } : t)),
      );

      try {
        const result = await runQuery(
          tab.serverId,
          tab.databaseName,
          query,
        );

        // Extract column names from QueryColumnInfo objects
        const fields = result.columns.map((col) => col.name);
        // Convert null values to empty strings for display
        const rows = result.rows.map((row) =>
          row.map((cell) => cell ?? ""),
        );

        setTabs((prevTabs) =>
          prevTabs.map((t) =>
            t.id === id
              ? { ...t, result: { rows, fields }, loading: false }
              : t,
          ),
        );
      } catch (error) {
        toast.error("Failed to execute query: " + String(error));

        setTabs((prevTabs) =>
          prevTabs.map((t) =>
            t.id === id
              ? {
                  ...t,
                  error: String(error),
                  loading: false,
                  result: { rows: [] },
                }
              : t,
          ),
        );
      }
    },
    [tabs, runQuery],
  );

  return {
    tabs,
    activeTab,
    loadingQuery,
    openTab,
    closeTab,
    setActiveTabId,
    setTabContent,
    setTabQuery,
    executeQuery,
  };
}
