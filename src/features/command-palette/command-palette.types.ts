import type { CatalogNodeKind, SchemaGroup } from '@/api/types/catalog.types';
import type { DatabaseType } from '@/api/types/server.types';

/** A single table: from the catalog (Postgres) or the structure cache (others). */
export interface TableEntry {
  /** encodeNodeId of the table — also the browse tab id */
  id: string;
  serverId: number;
  serverName: string;
  dbType: DatabaseType;
  database: string;
  /** null for schema-less databases (Mongo/Redis) */
  schema: string | null;
  table: string;
  /** Fuzzy target & display: `schema.table` or `table` */
  label: string;
}

interface DatabaseScope {
  serverId: number;
  serverName: string;
  dbType: DatabaseType;
  database: string;
}

/** O que um item faz ao ser escolhido. */
export type PaletteTarget =
  /** Abre a tabela */
  | { kind: 'table'; entry: TableEntry }
  /**
   * A mesma relação em vários schemas ("orders em 5.000 schemas"): escolher
   * refina a busca para `.orders`, com o cursor antes do ponto para o schema
   */
  | (DatabaseScope & {
      kind: 'group';
      name: string;
      relationKind: CatalogNodeKind;
      schemas: SchemaGroup;
    })
  /** Um schema: escolher lista as relações dele (`schema.`) */
  | (DatabaseScope & { kind: 'schema'; schema: string });

export interface PaletteItem {
  key: string;
  target: PaletteTarget;
  label: string;
  /** Indices in `label` to highlight */
  indices: number[];
  /** Secondary line below the label (already resolved for the group context) */
  subtitle: string;
}

export interface ResultGroup {
  key: string;
  heading: string;
  items: PaletteItem[];
}

/**
 * Flattened row model for the virtualized list — group headings and items are
 * interleaved into a single array so one virtualizer can scroll the whole list.
 */
export type PaletteRow =
  | { kind: 'header'; key: string; heading: string }
  | { kind: 'item'; key: string; item: PaletteItem };

/** Onde pôr o cursor do campo depois de o item reescrever a busca */
export interface QueryCaret {
  position: number;
  /** Muda a cada pedido, para o mesmo texto reposicionar de novo */
  nonce: number;
}
