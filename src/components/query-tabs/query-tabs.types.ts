import type { ReactNode } from 'react';
import type {
  CloseTab,
  SetActiveTabId,
} from '@/shared/hooks/use-query-tabs/use-query-tabs.types';
import type { Tab } from '@/shared/models/tabs.types';

export interface QueryTabsProps {
  tabs: Tab[];
  activeTabId?: string;
  onTabChange: SetActiveTabId;
  onTabClose: CloseTab;
  children: ReactNode;
}
