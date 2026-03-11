import type { Tab } from '@/shared/models/tabs.types';

export type OpenTab = (
  serverId: number,
  databaseName: string,
  initalData?: Partial<Tab>,
) => void;
export type CloseTab = (id: string) => void;
export type SetActiveTabId = (id: string) => void;
export type SetTabContent = (id: string, content: string) => void;
