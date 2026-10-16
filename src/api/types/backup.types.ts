// Gerados do Rust (ts-rs) — ver src-tauri/src/models; não edite à mão
export type { ArchiveSummary } from './generated/ArchiveSummary';
export type { BackupEvent } from './generated/BackupEvent';
export type { BackupOptions } from './generated/BackupOptions';
export type { BackupPreview } from './generated/BackupPreview';
export type { BackupRequest } from './generated/BackupRequest';
export type { BackupTarget } from './generated/BackupTarget';
export type { DumpContent } from './generated/DumpContent';
export type { DumpFormat } from './generated/DumpFormat';
export type { PgToolsDetection } from './generated/PgToolsDetection';
export type { PgToolsInstall } from './generated/PgToolsInstall';
export type { PgToolsProblem } from './generated/PgToolsProblem';
export type { PgToolsSource } from './generated/PgToolsSource';
export type { PgToolsStatus } from './generated/PgToolsStatus';
export type { RestoreFormat } from './generated/RestoreFormat';
export type { RestoreInspection } from './generated/RestoreInspection';
export type { RestoreOptions } from './generated/RestoreOptions';
export type { RestoreRequest } from './generated/RestoreRequest';

/**
 * Ferramentas nativas do Postgres (`pg_dump`, `pg_restore`, `psql`) usadas no
 * backup e na restauração — espelho de `src-tauri/src/models/backup.rs`.
 */

/** Evento global do Tauri com o andamento dos backups/restaurações */
export const BACKUP_EVENT = 'backup-event';

