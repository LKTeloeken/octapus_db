export interface Server {
  id: number;
  name: string;
  host: string;
  port: number;
  username: string;
  password: string;
  created_at?: number;
  isConnected?: boolean;
  default_database?: string;
}

export type ServerPrimitive = Omit<Server, 'id' | 'created_at'>;
