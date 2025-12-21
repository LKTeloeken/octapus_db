export interface QueryTab {
  id: string;
  serverId: number;
  databaseName: string;
  title: string;
  content: string;
  query?: string;
  active: boolean;
  loading?: boolean;
  result?: { rows: any[]; fields?: string[] };
}
