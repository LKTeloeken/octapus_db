import { IServer, IServerPrimitive } from "./server";
import { IPostgreSchema, IPostgreTable, IPostgreColumn } from "./postgreDb";

// Individual function types
export type FetchServersFunction = () => Promise<void>;

export type AddServerFunction = (
  data: IServerPrimitive
) => Promise<IServer | undefined>;

export type GetServerFunction = (id: number) => Promise<IServer | undefined>;

export type EditServerFunction = (
  id: number,
  data: IServerPrimitive
) => Promise<IServer | undefined>;

export type RemoveServerFunction = (id: number) => Promise<void>;

export type ConnectToServerFunction = (
  server: IServer
) => Promise<boolean | undefined>;

export type GetDatabaseSchemasFunction = (
  serverId: number,
  databaseName: string
) => Promise<IPostgreSchema[] | undefined>;

export type GetSchemaTablesFunction = (
  serverId: number,
  schemaName: string,
  databaseName: string
) => Promise<IPostgreTable[] | undefined>;

export type GetSchemaColumnsFunction = (
  serverId: number,
  schemaName: string,
  tableName: string,
  databaseName: string
) => Promise<IPostgreColumn[] | undefined>;

// Helper function types
export type BuildKeyFunction = (...parts: (string | number)[]) => string;

export type UpdateServerInTreeFunction = (
  serverId: number,
  data: any,
  remove?: boolean
) => void;

export type InitLoadingStateFunction = () => void;

export type HandleErrorFunction = (err: unknown) => void;
