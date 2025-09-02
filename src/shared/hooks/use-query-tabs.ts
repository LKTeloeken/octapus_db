import { createContext, useState } from "react";
import { runPostgreQuery } from "@/api/postgreMethods";

import type { userQueryTabsProps } from "@/shared/models/query-tabs";

interface QueryTab {
  id: string;
  serverId: number;
  databaseName: string;
  title: string;
  content: string;
  result?: { rows: any[]; fields?: string[] };
}

export function useQueryTabs({ servers }: userQueryTabsProps) {
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

  const runQuery = async (id: string) => {
    const tab = tabs.find((t) => t.id === id);
    if (!tab) return;
    const result = await runPostgreQuery(
      tab.serverId,
      tab.databaseName,
      tab.content
    );
    setTabs((prev) =>
      prev.map((t) => (t.id === id ? { ...t, result: { rows: result } } : t))
    );
  };
}
