import { memo } from "react";
import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Database, X } from "lucide-react";

import type { QueryTabsProps } from "./query-tabs.types";

export const QueryTabs = memo(
  ({ tabs, activeTabId, onTabChange, onTabClose }: QueryTabsProps) => {
    return (
      <Tabs
        defaultValue={activeTabId}
        value={activeTabId}
        onValueChange={onTabChange}
        className="max-w-xs w-full"
      >
        <TabsList className="p-1">
          {tabs.map((tab) => (
            <TabsTrigger key={tab.id} value={tab.id} className="px-2.5 sm:px-3">
              <code className="flex items-center gap-1 text-[13px] [&>svg]:h-4 [&>svg]:w-4">
                <Database /> {tab.title}
              </code>
              <span
                className="ml-2 inline-flex h-4 w-4 items-center justify-center rounded-sm hover:bg-accent hover:text-accent-foreground"
                onClick={(e) => {
                  e.stopPropagation();
                  onTabClose(tab.id);
                }}
              >
                <X className="h-3 w-3" />
              </span>
            </TabsTrigger>
          ))}
        </TabsList>
      </Tabs>
    );
  }
);
