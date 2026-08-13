export type FilterOp =
  | 'eq'
  | 'ne'
  | 'in'
  | 'like'
  | 'gt'
  | 'gte'
  | 'lt'
  | 'lte'
  | 'is_null'
  | 'not_null';

export interface ColumnFilter {
  column: string;
  op: FilterOp;
  /** 'in' uses all; comparisons use the first; is_null/not_null ignore */
  values: string[];
}

export interface SortSpec {
  column: string;
  direction: 'asc' | 'desc';
}

export interface TableDataRequest {
  /** Postgres: schema (default 'public'); Mongo/Redis: ignored */
  schema?: string | null;
  /** Table / collection / key group */
  table: string;
  filters?: ColumnFilter[];
  sort?: SortSpec[];
  limit?: number;
  offset?: number;
  countTotal?: boolean;
}
