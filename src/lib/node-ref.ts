/**
 * Typed identity for tree nodes and tabs. Replaces positional
 * `id.split('-')` parsing, which broke on names containing `-`.
 * Segments are URI-encoded, so any database/schema/table name is safe.
 */
export interface NodeRef {
  serverId: number;
  database?: string;
  schema?: string;
  table?: string;
  column?: string;
}

export type NodeKind = 'server' | 'database' | 'schema' | 'table' | 'column';

const SEGMENT_KEYS = ['database', 'schema', 'table', 'column'] as const;

const SERVER_PREFIX = 'sv';

// Two-letter prefixes so none collides with the server prefix — a single-letter
// scheme made `schema` ('s') clash with `serverId` ('s') and corrupted the ref.
const SEGMENT_PREFIX: Record<(typeof SEGMENT_KEYS)[number], string> = {
  database: 'db',
  schema: 'sc',
  table: 'tb',
  column: 'co',
};

export function nodeKind(ref: NodeRef): NodeKind {
  if (ref.column != null) return 'column';
  if (ref.table != null) return 'table';
  if (ref.schema != null) return 'schema';
  if (ref.database != null) return 'database';
  return 'server';
}

export function encodeNodeId(ref: NodeRef): string {
  const parts = [`${SERVER_PREFIX}=${ref.serverId}`];
  for (const key of SEGMENT_KEYS) {
    const value = ref[key];
    if (value != null) {
      parts.push(`${SEGMENT_PREFIX[key]}=${encodeURIComponent(value)}`);
    }
  }
  return parts.join('|');
}

export function decodeNodeId(id: string): NodeRef {
  const ref: NodeRef = { serverId: -1 };

  for (const part of id.split('|')) {
    const eq = part.indexOf('=');
    if (eq === -1) continue;

    const prefix = part.slice(0, eq);
    const value = part.slice(eq + 1);

    if (prefix === SERVER_PREFIX) {
      ref.serverId = Number(value);
      continue;
    }

    const key = SEGMENT_KEYS.find(k => SEGMENT_PREFIX[k] === prefix);
    if (key) ref[key] = decodeURIComponent(value);
  }

  return ref;
}
