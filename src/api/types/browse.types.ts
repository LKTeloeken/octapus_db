export interface SortSpec {
  column: string;
  direction: 'asc' | 'desc';
}

export interface TableDataRequest {
  /** Postgres: schema (default 'public'); Mongo/Redis: ignored */
  schema?: string | null;
  /** Table / collection / key group */
  table: string;
  /** Postgres-only raw WHERE expression (without the WHERE keyword) */
  whereExpr?: string;
  sort?: SortSpec[];
  limit?: number;
  offset?: number;
  countTotal?: boolean;
}
