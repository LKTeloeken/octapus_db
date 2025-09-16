export interface QueryTabsProps {
  tabs: QueryTab[];
  activeTabId: string;
  onTabChange: (tabId: string) => void;
  onTabClose: (tabId: string) => void;
}

type QueryTab = {
  id: string;
  title: string;
};
