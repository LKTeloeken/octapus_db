export type DatabaseType = 'postgres' | 'mongodb' | 'redis' | 'sqlite';

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
  /**
   * Escopo salvo: que databases o app enxerga (padrões com `*` e `?`, `!` na
   * frente exclui; separados por vírgula). null = todos
   */
  scopeDatabases: string | null;
  /** Escopo salvo: que schemas o catálogo lê (Postgres). null = todos */
  scopeSchemas: string | null;
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
  scopeDatabases?: string | null;
  scopeSchemas?: string | null;
}

export interface PoolStats {
  size: number;
  available: number;
  inUse: number;
  waiting: number;
}
