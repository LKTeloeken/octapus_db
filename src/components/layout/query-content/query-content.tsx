import { useMemo, useRef } from "react";
import {
  ResizablePanel,
  ResizablePanelGroup,
  ResizableHandle,
} from "@/components/ui/resizable";
import { Button } from "@/components/ui/button";
import { Play } from "lucide-react";
import QueryTabs from "@/components/layout/query-tabs/query-tabs";
import SQLEditor from "@/components/layout/query-editor/query-editor";
import { useTabs } from "@/shared/providers/tabs-provider";

import type { FC } from "react";

import QueryResults from "../QueryResults";

const QueryContent: FC = () => {
  const { tabs, activeTabId, setActiveTabId, setContent } = useTabs();
  const textBaseRef = useRef<HTMLTextAreaElement>(null);

  const activeTab = useMemo(() => {
    return tabs.find((tab) => tab.id === activeTabId);
  }, [activeTabId, tabs]);

  const onChange = (newValue: string) => {
    if (!activeTab) return;
    setContent(activeTab.id, newValue);
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

              <Button variant="default" size="sm">
                <Play />
                Run query
              </Button>
            </div>

            <div
              className="h-full w-full bg-violet-300/20 rounded-lg"
              onClick={() => textBaseRef.current?.focus()}
            >
              <SQLEditor
                value={activeTab?.content || ""}
                onChange={onChange}
                customRef={textBaseRef}
              />
            </div>
          </ResizablePanel>

          <ResizableHandle withHandle />

          <ResizablePanel defaultSize={60} minSize={20}>
            {activeTab.result && <QueryResults />}
            {!activeTab.result && (
              <div className="flex h-full w-full items-center justify-center">
                <p className="text-muted-foreground">No results to show</p>
              </div>
            )}
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
