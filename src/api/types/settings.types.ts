/**
 * Preferências do app, gravadas no SQLite local — espelho de
 * `src-tauri/src/models/settings.rs`. Campo novo nos dois lados, com o padrão
 * no `Default` do Rust (o que nunca foi salvo volta com ele).
 */
export interface AppSettings {
  /** Recebe também as pre-releases do GitHub no auto-update (canal beta) */
  betaUpdates: boolean;
}
