import type { ReactNode } from "react";
import type { QueryTab } from "@/shared/models/query-tabs.types";
import type {
  CloseTab,
  SetActiveTabId,
} from "@/shared/hooks/use-query-tabs/use-query-tabs.types";

export interface QueryTabsProps {
  tabs: QueryTab[];
  activeTabId?: string;
  onTabChange: SetActiveTabId;
  onTabClose: CloseTab;
  children: ReactNode;
}
