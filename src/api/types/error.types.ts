/** Espelha `Error::code()` em src-tauri/src/error.rs */
export type BackendErrorCode =
  | 'STORAGE_ERROR'
  | 'CONNECTION_ERROR'
  | 'POOL_EXHAUSTED'
  | 'CONNECTION_TIMEOUT'
  | 'QUERY_ERROR'
  | 'INVALID_QUERY'
  | 'NOT_FOUND'
  | 'ALREADY_EXISTS'
  | 'INVALID_STATE'
  | 'CANCELLED'
  | 'UNSUPPORTED_DATABASE'
  | 'UNSUPPORTED_TYPE'
  | 'EXTERNAL_ERROR';

/** Rejeição de um comando do app: o `Serialize` de `Error` no Rust */
export interface BackendError {
  code: BackendErrorCode;
  message: string;
}
