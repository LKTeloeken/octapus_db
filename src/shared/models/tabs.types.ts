export interface Tab {
  id: string;
  serverId: number;
  databaseName: string;
  title: string;
  content: string;
  active: boolean;
  type: TabType;
}

export enum TabType {
  View = 'view',
  Query = 'query',
}
