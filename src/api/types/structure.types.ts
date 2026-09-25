export interface DatabaseInfo {
  name: string;
  sizeBytes: number | null;
}

export interface SchemaInfo {
  name: string;
  tableCount: number | null;
}

export type TableType = 'table' | 'view' | 'materializedview' | 'foreign';

export interface TableInfo {
  name: string;
  schema: string;
  tableType: TableType;
  rowEstimate: number | null;
  /** Dados + índices em disco; null em views e em Mongo/Redis/SQLite */
  sizeBytes: number | null;
}

export interface ColumnInfo {
  name: string;
  ordinal: number;
  dataType: string;
  isNullable: boolean;
  defaultValue: string | null;
  isPrimaryKey: boolean;
  isForeignKey: boolean;
}

export interface IndexInfo {
  name: string;
  columns: string[];
  isUnique: boolean;
  isPrimary: boolean;
  indexType: string;
}

export interface DatabaseStructure {
  schemas: {
    name: string;
    tables: {
      name: string;
      tableType: TableType;
      /** Dados + índices em disco; null em views e em Redis/SQLite */
      sizeBytes: number | null;
    }[];
  }[];
  /** Epoch in ms */
  fetchedAt: number;
}
