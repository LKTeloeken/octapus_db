/**
 * Códigos da API do assistente (`AssistantCode::app_code` em
 * src-tauri/src/assistant/error.rs): o `ErrorCode` do contrato com o prefixo
 * `ASSISTANT_`. `ASSISTANT_UNAVAILABLE` também cobre a API inalcançável.
 */
export const ASSISTANT_API_ERROR_CODES = [
  'ASSISTANT_INVALID_REQUEST',
  'ASSISTANT_UNAUTHORIZED',
  'ASSISTANT_CONSENT_REQUIRED',
  'ASSISTANT_MODEL_SELECTION_FORBIDDEN',
  'ASSISTANT_NOT_FOUND',
  'ASSISTANT_CONFLICT',
  'ASSISTANT_PROMPT_NOT_REGISTERED',
  'ASSISTANT_QUOTA_EXCEEDED',
  'ASSISTANT_TURN_LIMIT_EXCEEDED',
  'ASSISTANT_MODEL_UNAVAILABLE',
  'ASSISTANT_INVALID_OUTPUT',
  'ASSISTANT_MODEL_REFUSED',
  'ASSISTANT_PROVIDER_ERROR',
  'ASSISTANT_RATE_LIMITED',
  'ASSISTANT_UNAVAILABLE',
  'ASSISTANT_INTERNAL',
] as const;

export type AssistantApiErrorCode = (typeof ASSISTANT_API_ERROR_CODES)[number];

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
  | 'EXTERNAL_ERROR'
  /** O guard do assistente recusou a consulta antes de ela ir ao banco */
  | 'GUARD_REJECTED'
  /** Build sem a URL da API do assistente */
  | 'ASSISTANT_NOT_CONFIGURED'
  | AssistantApiErrorCode;

/** Rejeição de um comando do app: o `Serialize` de `Error` no Rust */
export interface BackendError {
  code: BackendErrorCode;
  message: string;
}
