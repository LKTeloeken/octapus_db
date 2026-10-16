import type {
  BackendError,
  BackendErrorCode,
} from '@/api/types/error.types';

/**
 * Prefixos do `Display` de `Error` (src-tauri/src/error.rs). Os handlers lançam
 * a mensagem já no formato do backend ("Query error: …"); o código sai dela.
 */
const PREFIXES: [string, BackendErrorCode][] = [
  ['Storage error:', 'STORAGE_ERROR'],
  ['Connection error:', 'CONNECTION_ERROR'],
  ['Connection pool exhausted', 'POOL_EXHAUSTED'],
  ['Connection timed out', 'CONNECTION_TIMEOUT'],
  ['Query error:', 'QUERY_ERROR'],
  ['Invalid query:', 'INVALID_QUERY'],
  ['Not found:', 'NOT_FOUND'],
  ['Already exists:', 'ALREADY_EXISTS'],
  ['Invalid state:', 'INVALID_STATE'],
  ['Operation cancelled', 'CANCELLED'],
  ['Unsupported database:', 'UNSUPPORTED_DATABASE'],
  ['Unsupported type:', 'UNSUPPORTED_TYPE'],
];

/**
 * O que um comando Rust rejeitaria: `{ code, message }`. Mensagem sem prefixo
 * conhecido é `External`, a única variante que sai sem prefixo.
 */
export function toBackendError(thrown: unknown): BackendError {
  const message =
    typeof thrown === 'string'
      ? thrown
      : thrown instanceof Error
        ? thrown.message
        : String(thrown);
  const code =
    PREFIXES.find(([prefix]) => message.startsWith(prefix))?.[1] ??
    'EXTERNAL_ERROR';
  return { code, message };
}
