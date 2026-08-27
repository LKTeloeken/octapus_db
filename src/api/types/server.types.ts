export type DatabaseType = 'postgres' | 'mongodb' | 'redis';

export interface Server {
  id: number;
  name: string;
  dbType: DatabaseType;
  host: string;
  port: number;
  username: string;
  defaultDatabase: string | null;
  sslEnabled: boolean;
  /**
   * URI **redigida**: o backend troca a senha embutida por `••••` antes de
   * serializar. Devolva-a intacta no `ServerInput` para preservar a credencial
   * guardada — qualquer valor diferente é tratado como uma URI nova.
   */
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
  /** Criptografada no cofre do backend — nunca volta no `Server` */
  password: string;
  defaultDatabase?: string | null;
  sslEnabled?: boolean | null;
  /** Criptografada no cofre. Reenvie a versão redigida do `Server` para mantê-la */
  connectionUri?: string | null;
}

export interface PoolStats {
  size: number;
  available: number;
  inUse: number;
  waiting: number;
}
