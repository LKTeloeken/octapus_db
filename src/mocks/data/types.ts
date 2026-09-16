import type {
  ColumnInfo,
  IndexInfo,
  TableType,
} from '@/api/types/structure.types';
import type { QueryColumnInfo } from '@/api/types/query.types';
import type { AdapterCapabilities } from '@/api/types/capabilities.types';
import type { Server } from '@/api/types/server.types';
import type { CellGen } from './rows';

/**
 * Uma coluna do mock carrega, num objeto só, o que o backend devolve em dois
 * formatos diferentes: `ColumnInfo` (list_columns, para a árvore) e
 * `QueryColumnInfo` (dentro do QueryResult, para o grid).
 */
export interface MockColumn {
  name: string;
  /** `ColumnInfo.dataType` — nome do tipo como o catálogo mostra */
  dataType: string;
  /** `QueryColumnInfo.typeName`; por padrão igual ao dataType */
  typeName?: string;
  /** Postgres traz o oid; Mongo/Redis sempre null */
  typeOid: number | null;
  isNullable: boolean;
  defaultValue: string | null;
  isPrimaryKey: boolean;
  isForeignKey: boolean;
  /** Como a célula é gerada na materialização da tabela */
  gen: CellGen;
}

export interface MockTable {
  name: string;
  /** '' quando o adapter não tem schemas (Mongo/Redis) */
  schema: string;
  tableType: TableType;
  columns: MockColumn[];
  indexes: IndexInfo[];
  /** Quantas linhas materializar; vira `TableInfo.rowEstimate` */
  rowEstimate: number;
  /** Materializado no primeiro acesso e mutável a partir daí */
  rows?: (string | null)[][];
  /** Próximo valor de chave sequencial, para os inserts do mock */
  nextSerial?: number;
}

export interface MockDatabase {
  name: string;
  sizeBytes: number | null;
  tables: MockTable[];
}

export interface MockServerEntry {
  server: Server;
  capabilities: AdapterCapabilities;
  databases: MockDatabase[];
}

export const toColumnInfo = (
  column: MockColumn,
  index: number,
): ColumnInfo => ({
  name: column.name,
  ordinal: index + 1,
  dataType: column.dataType,
  isNullable: column.isNullable,
  defaultValue: column.defaultValue,
  isPrimaryKey: column.isPrimaryKey,
  isForeignKey: column.isForeignKey,
});

export const toQueryColumnInfo = (column: MockColumn): QueryColumnInfo => ({
  name: column.name,
  typeName: column.typeName ?? column.dataType,
  typeOid: column.typeOid,
});

export const primaryKeyOf = (table: MockTable): string[] =>
  table.columns
    .filter(column => column.isPrimaryKey)
    .map(column => column.name);
