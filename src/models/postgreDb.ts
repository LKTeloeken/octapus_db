export interface IPostgreServer {
  id: number;
  name: String;
  host: String;
  port: number;
  username: String;
  password: String;
  created_at: number;
  isConnected: boolean;
}

export type IPostgreServerPrimitive = Omit<IPostgreServer, "id" | "created_at">;

export interface IPostgreSchema {
  name: string;
  tables: IPostgreTable[];
  views: IPostgreView[];
  sequences: IPostgreSequence[];
}

export interface IPostgreTable {
  name: string;
  schema: string;
  table_type: string;
  triggers: IPostgreTrigger[];
  columns: IPostgreColumn[];
  indexes: IPostgreIndex[];
  primary_keys: IPostgrePrimaryKey[];
  foreign_keys: IPostgreForeignKey[];
}

export interface IPostgreColumn {
  name: string;
  ordinal_position: number;
  data_type: string;
  is_nullable: boolean;
  column_default: string | null;
}

export interface IPostgreTrigger {
  schema_name: string;
  table_name: string;
  name: string;
  action_timing: string;
  events: string[];
  action_statement: string;
}

export interface IPostgreIndex {
  schema_name: string;
  table_name: string;
  name: string;
  index_def: string;
}

export interface IPostgrePrimaryKey {
  constraint_name: string;
  column_name: string;
  ordinal_position: number;
}

export interface IPostgreForeignKey {
  constraint_name: string;
  column_name: string;
  table_schema: string;
  table_name: string;
}

export interface IPostgreView {
  name: string;
  schema: string;
  definition: string;
}

export interface IPostgreSequence {
  schema: string;
  name: string;
  data_type: string;
  start_value: number;
  minimum_value: number;
  maximum_value: number;
  increment: number;
  cycle_option: boolean;
}
