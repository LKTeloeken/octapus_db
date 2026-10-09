import { FLAT_SCHEMA, type CatalogSearchHit } from '@/api/types/catalog.types';
import type { Server } from '@/api/types/server.types';
import { formatCount } from '@/lib/format-count';
import { fuzzyMatch } from '@/lib/fuzzy';
import { encodeNodeId } from '@/lib/node-ref';
import type {
  PaletteItem,
  PaletteRow,
  ResultGroup,
  TableEntry,
} from './command-palette.types';

type ServerInfo = Pick<Server, 'id' | 'name' | 'dbType'>;

/** Posições a destacar no rótulo (o backend devolve só a pontuação). */
function highlight(query: string, label: string): number[] {
  const match = fuzzyMatch(query, label);
  return match.matched ? match.indices : [];
}

/** O schema sem nome de Mongo e SQLite (`FLAT_SCHEMA`) vira "sem schema". */
export function tableEntry(
  server: ServerInfo,
  database: string,
  catalogSchema: string | null,
  table: string,
): TableEntry {
  const schema = catalogSchema === FLAT_SCHEMA ? null : catalogSchema;
  return {
    id: encodeNodeId({
      serverId: server.id,
      database,
      schema: schema ?? undefined,
      table,
    }),
    serverId: server.id,
    serverName: server.name,
    dbType: server.dbType,
    database,
    schema,
    table,
    label: schema ? `${schema}.${table}` : table,
  };
}

/**
 * Um resultado do catálogo vira um item: a relação de um schema só (ou de um
 * par `schema.tabela`) abre direto; a mesma relação em vários schemas vira um
 * grupo que refina a busca; um schema lista as relações dele.
 */
export function catalogHitToItem(
  hit: CatalogSearchHit,
  query: string,
  server: ServerInfo | undefined,
): PaletteItem | null {
  if (!server) return null;
  const scope = {
    serverId: server.id,
    serverName: server.name,
    dbType: server.dbType,
    database: hit.database,
  };

  if (hit.kind === 'schema') {
    return {
      key: `schema|${hit.serverId}|${hit.database}|${hit.name}`,
      target: { ...scope, kind: 'schema', schema: hit.name },
      label: hit.name,
      indices: highlight(query, hit.name),
      subtitle: `schema · ${hit.database}`,
    };
  }

  // `FLAT_SCHEMA` é string vazia: comparar com null, não pela verdade
  const only =
    hit.schema ?? (hit.schemas?.total === 1 ? hit.schemas.sample[0] : null);
  if (only != null) {
    const entry = tableEntry(server, hit.database, only, hit.name);
    return {
      key: entry.id,
      target: { kind: 'table', entry },
      label: entry.label,
      indices: highlight(query, entry.label),
      subtitle: hit.database,
    };
  }

  const schemas = hit.schemas ?? { total: 0, sample: [] };
  return {
    key: `group|${hit.serverId}|${hit.database}|${hit.name}`,
    target: {
      ...scope,
      kind: 'group',
      name: hit.name,
      relationKind: hit.kind,
      schemas,
    },
    label: hit.name,
    indices: highlight(query, hit.name),
    subtitle: `em ${formatCount(schemas.total)} schemas · ${hit.database}`,
  };
}

/** Agrupa por servidor mantendo a ordem de chegada (melhores primeiro). */
export function groupByServer(items: PaletteItem[]): ResultGroup[] {
  const byServer = new Map<number, ResultGroup>();
  for (const item of items) {
    const { serverId, serverName } =
      item.target.kind === 'table' ? item.target.entry : item.target;
    let group = byServer.get(serverId);
    if (!group) {
      group = { key: `server-${serverId}`, heading: serverName, items: [] };
      byServer.set(serverId, group);
    }
    group.items.push(item);
  }
  return Array.from(byServer.values());
}

/** Títulos de grupo intercalados com os itens, para um virtualizador só. */
export function flattenGroups(groups: ResultGroup[]): PaletteRow[] {
  const rows: PaletteRow[] = [];
  for (const group of groups) {
    rows.push({ kind: 'header', key: group.key, heading: group.heading });
    for (const item of group.items)
      rows.push({ kind: 'item', key: item.key, item });
  }
  return rows;
}

/**
 * Com um schema fixado, o item mostra só a tabela (o schema está no chip do
 * campo) e o destaque segue o que foi digitado.
 */
export function pinnedTableItem(entry: TableEntry, query: string): PaletteItem {
  return {
    key: entry.id,
    target: { kind: 'table', entry },
    label: entry.table,
    indices: highlight(query, entry.table),
    subtitle: `${entry.schema} · ${entry.database}`,
  };
}

/**
 * O que o Tab fixa: o schema do item ativo, se for um schema; senão o trecho
 * antes do ponto (`public.us` fixa `public` e segue buscando `us`); senão o
 * texto todo. `null` quando não há o que fixar.
 */
export function resolvePin(
  query: string,
  active: PaletteItem | null,
): { schema: string; rest: string } | null {
  const dot = query.indexOf('.');
  if (active?.target.kind === 'schema') {
    return {
      schema: active.target.schema,
      rest: dot === -1 ? '' : query.slice(dot + 1).trim(),
    };
  }
  if (dot !== -1) {
    const schema = query.slice(0, dot).trim();
    return schema ? { schema, rest: query.slice(dot + 1).trim() } : null;
  }
  const schema = query.trim();
  return schema ? { schema, rest: '' } : null;
}

/** O schema de uma entrada é o fixado? (maiúsculas não importam, como no backend) */
export function inPinnedSchema(entry: TableEntry, schema: string): boolean {
  return entry.schema?.toLowerCase() === schema.toLowerCase();
}
