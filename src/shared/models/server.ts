import { IPostgreDatabase } from "./postgreDb";

export interface IServer {
  id: number;
  name: string;
  host: string;
  port: number;
  username: string;
  password: string;
  created_at: number;
  isConnected: boolean;
  default_database?: string;
  databases?: IPostgreDatabase[];
}

export type IServerPrimitive = Omit<IServer, "id" | "created_at">;
