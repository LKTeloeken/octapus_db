import { useState } from "react";
import { useRunQuery } from "./use-run-query";

import type { QueryTab } from "../models/query-tabs";

export function useQueryTabs() {
  const { runQuery } = useRunQuery();
  const [tabs, setTabs] = useState<QueryTab[]>([]);
  const [activeTabId, setActiveTabId] = useState<string>();

  const openTab = (serverId: number, db: string) => {
    const id = `${serverId}-${db}-${Date.now()}`;
    setTabs((prev) => [
      ...prev,
      { id, serverId, databaseName: db, title: db, content: "" },
    ]);
    setActiveTabId(id);
  };

  const closeTab = (id: string) => {
    setTabs((prev) => prev.filter((t) => t.id !== id));
    if (activeTabId === id) setActiveTabId(undefined);
  };

  const setContent = (id: string, content: string) => {
    setTabs((prev) => prev.map((t) => (t.id === id ? { ...t, content } : t)));
  };

  const executeQuery = async (id: string, query: string) => {
    const tab = tabs.find((t) => t.id === id);
    if (!tab) return;
    const { rows, fields } = await runQuery(
      tab.serverId,
      tab.databaseName,
      query
    );

    setTabs((prev) =>
      prev.map((t) => (t.id === id ? { ...t, result: { rows, fields } } : t))
    );
  };

  return {
    tabs,
    activeTabId,
    openTab,
    closeTab,
    setActiveTabId,
    setContent,
    executeQuery,
  };
}
