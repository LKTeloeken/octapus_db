export interface QueryTab {
  id: string;
  serverId: number;
  databaseName: string;
  title: string;
  content: string;
  active: boolean;
  loading?: boolean;
  result?: { rows: any[]; fields?: string[] };
}
