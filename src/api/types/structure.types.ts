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

// ── Edição da estrutura pela árvore (Postgres e SQLite) ─────────────────────

/** Uma coluna como o diálogo descreve; tipo e default são SQL do próprio banco */
export interface ColumnDefinition {
  name: string;
  dataType: string;
  nullable: boolean;
  defaultValue: string | null;
  /** Só na criação (tabela ou coluna nova) */
  primaryKey: boolean;
}

export type RelationKind = 'table' | 'view' | 'materializedView' | 'foreign';

/** `schema` é null nos bancos sem schema (SQLite) */
export type StructureChange =
  | {
      type: 'createTable';
      schema: string | null;
      table: string;
      columns: ColumnDefinition[];
    }
  | {
      type: 'renameTable';
      schema: string | null;
      table: string;
      newName: string;
    }
  | {
      type: 'dropTable';
      schema: string | null;
      table: string;
      kind: RelationKind;
      /** Postgres: leva junto o que depende dela */
      cascade: boolean;
    }
  | {
      type: 'addColumn';
      schema: string | null;
      table: string;
      column: ColumnDefinition;
    }
  | {
      type: 'alterColumn';
      schema: string | null;
      table: string;
      /** Como a coluna estava: só o que mudou vira statement */
      original: ColumnDefinition;
      updated: ColumnDefinition;
    }
  | {
      type: 'dropColumn';
      schema: string | null;
      table: string;
      column: string;
      cascade: boolean;
    };
