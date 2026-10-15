import type {
  BackupOptions,
  BackupTarget,
  DumpContent,
  DumpFormat,
  PgToolsStatus,
  RestoreFormat,
  RestoreOptions,
} from '@/api/types/backup.types';
import { describePgToolsProblem } from '@/lib/pg-tools';

/** Padrão do diálogo: `.dump` completo, pronto para outro servidor */
export const DEFAULT_BACKUP_OPTIONS: BackupOptions = {
  format: 'custom',
  content: 'all',
  noOwner: true,
  clean: false,
  columnInserts: false,
};

export const FORMAT_OPTIONS: { value: DumpFormat; label: string }[] = [
  { value: 'custom', label: 'Backup (.dump)' },
  { value: 'plain', label: 'Script SQL (.sql)' },
];

export const FORMAT_HINTS: Record<DumpFormat, string> = {
  custom: 'Compactado. Restaura pelo Octapus ou pelo pg_restore.',
  // O pg_dump 17.6+ escreve `\restrict` no script: comando do psql, que
  // outros clientes SQL não entendem
  plain: 'Texto legível. Restaurado pelo Octapus ou pelo psql.',
};

export const CONTENT_OPTIONS: { value: DumpContent; label: string }[] = [
  { value: 'all', label: 'Estrutura e dados' },
  { value: 'schemaOnly', label: 'Só estrutura' },
  { value: 'dataOnly', label: 'Só dados' },
];

/**
 * Desliga o que deixou de valer depois de uma troca: "apagar antes" e
 * "INSERT" só existem no script SQL, e cada um briga com um dos conteúdos
 * (o backend recusaria a combinação).
 */
export function normalizeBackupOptions(options: BackupOptions): BackupOptions {
  const plain = options.format === 'plain';
  return {
    ...options,
    clean: options.clean && plain && options.content !== 'dataOnly',
    columnInserts:
      options.columnInserts && plain && options.content !== 'schemaOnly',
  };
}

/** Nome do alvo para títulos e para o card de progresso */
export function backupTargetLabel(
  database: string,
  target: BackupTarget,
): string {
  switch (target.kind) {
    case 'database':
      return database;
    case 'schema':
      return `${database} › ${target.schema}`;
    case 'table':
      return `${database} › ${target.schema}.${target.table}`;
  }
}

/** Padrão da restauração: sem dono do arquivo, sem apagar nada, tudo ou nada */
export const DEFAULT_RESTORE_OPTIONS: RestoreOptions = {
  noOwner: true,
  clean: false,
  singleTransaction: true,
};

/**
 * O que vale para o formato: no script SQL, dono, permissões e DROP são os que
 * estão escritos nele (o backend recusaria as opções).
 */
export function effectiveRestoreOptions(
  format: RestoreFormat,
  options: RestoreOptions,
): RestoreOptions {
  return format === 'plain'
    ? { ...options, noOwner: false, clean: false }
    : options;
}

/** O que impede a restauração, do lado das ferramentas; null se nada */
export function restoreToolsProblem(
  format: RestoreFormat | null,
  status: PgToolsStatus | undefined,
): string | null {
  if (!status) return null;
  const { problem, selected } = status;
  // pg_restore/psql mais antigos que o servidor costumam funcionar; o que não
  // funciona (arquivo de versão mais nova) aparece na leitura do arquivo
  if (problem && problem.kind !== 'clientTooOld') {
    return describePgToolsProblem(problem);
  }
  if (format === 'custom' && !selected?.pgRestore) {
    return 'Não há pg_restore na pasta das ferramentas do PostgreSQL — ele é preciso para ler arquivos .dump.';
  }
  if (format === 'plain' && !selected?.psql) {
    return 'Não há psql na pasta das ferramentas do PostgreSQL — ele é preciso para rodar scripts .sql.';
  }
  return null;
}
