export type DatabaseType =
  | 'postgres'
  | 'mongodb'
  | 'redis'
  | 'mysql'
  | 'sqlite';

export interface Server {
  id: number;
  name: string;
  dbType: DatabaseType;
  host: string;
  port: number;
  username: string;
  defaultDatabase: string | null;
  sslEnabled: boolean;
  connectionUri: string | null;
  /** Epoch in seconds */
  createdAt: number;
}

export interface ServerInput {
  name: string;
  dbType: DatabaseType;
  host: string;
  port: number;
  username: string;
  /** Stored in the OS keychain by the backend — never returned on Server */
  password: string;
  defaultDatabase?: string | null;
  sslEnabled?: boolean | null;
  connectionUri?: string | null;
}

export interface PoolStats {
  size: number;
  available: number;
  inUse: number;
  waiting: number;
}
