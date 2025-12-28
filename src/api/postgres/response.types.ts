export type ConnectToServerResponse = boolean;

export interface ExecutePostgreQueryResponse {
  rows: string[][];
  columns: { name: string; type_name: string }[];
  row_count: number | null;
  total_count: number | null;
  has_more: boolean;
}

export interface ExecuteQueryResponse {
  rows: string[][];
  fields: string[];
}
