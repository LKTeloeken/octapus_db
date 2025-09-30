import { useMemo, useRef, useState } from "react";
import {
  ResizablePanel,
  ResizablePanelGroup,
  ResizableHandle,
} from "@/components/ui/resizable";
import { Button } from "@/components/ui/button";
import { Play } from "lucide-react";
import QueryTabs from "@/components/layout/query-tabs/query-tabs";
import SQLEditor from "@/components/layout/query-editor/query-editor";
import { QueryResultsTable } from "../query-results/query-results";
import { useTabs } from "@/shared/providers/tabs-provider";

import type { FC } from "react";

const QueryContent: FC = () => {
  const {
    tabs,
    activeTabId,
    loadingQuery,
    setActiveTabId,
    setContent,
    executeQuery,
  } = useTabs();
  const [query, setQuery] = useState<string>("");

  const activeTab = useMemo(() => {
    return tabs.find((tab) => tab.id === activeTabId);
  }, [activeTabId, tabs]);

  const onChange = (newValue: string) => {
    if (!activeTab) return;
    setContent(activeTab.id, newValue);
  };

  const onChangeSelection = (selection: { start: number; end: number }) => {
    const _query =
      activeTab?.content.slice(selection.start, selection.end) || "";

    setQuery(_query);
  };

  const runQuery = () => {
    if (!activeTab) return;
    executeQuery(activeTab.id, query || activeTab.content);
  };

  return (
    <ResizablePanel defaultSize={80} className="overflow-hidden">
      {activeTab && (
        <ResizablePanelGroup direction="vertical" className="h-full w-full">
          <ResizablePanel
            defaultSize={40}
            minSize={20}
            maxSize={80}
            className="border-b border-border p-2 flex flex-col gap-2"
          >
            <div className="flex items-center justify-between">
              <QueryTabs
                tabs={tabs}
                activeTabId={activeTabId?.toString() || ""}
                onTabChange={setActiveTabId}
                onTabClose={() => {}}
              />

              <Button variant="default" size="sm" onClick={runQuery}>
                <Play />
                Run query
              </Button>
            </div>

            <div className="h-full w-full bg-violet-300/20 rounded-lg">
              <SQLEditor
                value={activeTab?.content || ""}
                onChange={onChange}
                onChangeSelection={onChangeSelection}
              />
            </div>
          </ResizablePanel>

          <ResizableHandle withHandle />

          <ResizablePanel defaultSize={60} minSize={20}>
            <QueryResultsTable
              rows={activeTab.result?.rows || []}
              columns={activeTab.result?.fields || []}
              emptyMessage="No results yet"
            />
          </ResizablePanel>
        </ResizablePanelGroup>
      )}

      {!activeTab && (
        <div className="flex h-full w-full items-center justify-center">
          <p className="text-muted-foreground">No tab selected</p>
        </div>
      )}
    </ResizablePanel>
  );
};

export default QueryContent;
