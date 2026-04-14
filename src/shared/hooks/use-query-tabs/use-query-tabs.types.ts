import type { RowEdit } from '@/api/database/database-responses.types';
import type { Tab } from '@/shared/models/tabs.types';

export type OpenTab = (
  serverId: number,
  databaseName: string,
  initialData?: Partial<Tab>,
) => void;
export type CloseTab = (id: string) => void;
export type SetActiveTabId = (id: string) => void;
export type SetTabContent = (id: string, content: string) => void;
export type ExecuteQuery = (id: string, query: string) => Promise<void>;
export type ApplyQueryTabChanges = (
  id: string,
  edits: RowEdit[],
) => Promise<void>;
