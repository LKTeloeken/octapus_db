export interface DatabaseProcess {
  pid: number;
  database: string | null;
  username: string | null;
  applicationName: string | null;
  clientAddress: string | null;
  clientPort: number | null;
  state: string | null;
  query: string;
  queryStart: string | null;
  transactionStart: string | null;
  backendStart: string | null;
  waitEventType: string | null;
  waitEvent: string | null;
  backendType: string;
  durationMs: number | null;
  isOwnProcess: boolean;
}
