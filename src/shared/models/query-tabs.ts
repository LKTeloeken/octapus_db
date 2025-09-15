import { IServer } from "./server";

export interface userQueryTabsProps {
  servers: IServer[];
}

export interface QueryTab {
  id: string;
  serverId: number;
  databaseName: string;
  title: string;
  content: string;
  result?: { rows: any[]; fields?: string[] };
}
