import type { ImportSource } from './generated/ImportSource';

// Gerados do Rust (ts-rs) — ver src-tauri/src/models; não edite à mão
export type { ImportCandidate } from './generated/ImportCandidate';
export type { ImportCommitRequest } from './generated/ImportCommitRequest';
export type { ImportPreview } from './generated/ImportPreview';
export type { ImportResult } from './generated/ImportResult';
export type { ImportSource } from './generated/ImportSource';
export type { ImportSourceInfo } from './generated/ImportSourceInfo';
export type { ImportWarning } from './generated/ImportWarning';
export type { PasswordStorage } from './generated/PasswordStorage';

export interface ImportPreviewRequest {
  source: ImportSource;
  /** Arquivo escolhido à mão; null = lugar padrão */
  path?: string | null;
  /** URLs coladas (origem `urls`) */
  text?: string | null;
}
