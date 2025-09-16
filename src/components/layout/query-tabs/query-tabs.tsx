import { Tabs, TabsList, TabsTrigger } from "@/components/ui/tabs";
import { Database } from "lucide-react";

import type { QueryTabsProps } from "./query-tabs.types";

export default function QueryTabs({
  tabs,
  activeTabId,
  onTabChange,
  onTabClose,
}: QueryTabsProps) {
  return (
    <Tabs
      defaultValue={activeTabId}
      value={activeTabId}
      onValueChange={onTabChange}
      className="max-w-xs w-full"
    >
      <TabsList className="p-1">
        {tabs.map((tab) => (
          <TabsTrigger
            key={tab.id}
            value={tab.id}
            // onClick={() => onTabChange(tab.id)}
            className="px-2.5 sm:px-3"
          >
            <code className="flex items-center gap-1 text-[13px] [&>svg]:h-4 [&>svg]:w-4">
              <Database /> {tab.title}
            </code>
          </TabsTrigger>
        ))}
      </TabsList>
    </Tabs>
  );
}
