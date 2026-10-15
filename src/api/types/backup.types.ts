/**
 * Ferramentas nativas do Postgres (`pg_dump`, `pg_restore`, `psql`) usadas no
 * backup e na restauração — espelho de `src-tauri/src/models/backup.rs`.
 */

/** De onde veio a instalação: Preferências, `PATH` ou local conhecido */
export type PgToolsSource = 'configured' | 'path' | 'knownLocation';

/** Uma pasta com o `pg_dump` (e, se houver, `pg_restore` e `psql`) */
export interface PgToolsInstall {
  dir: string;
  source: PgToolsSource;
  /** Como o `pg_dump --version` escreve (`16.4`, `17beta1`) */
  version: string;
  /** No formato do `server_version_num` (160004 = 16.4) */
  versionNum: number;
  pgDump: string;
  pgRestore: string | null;
  psql: string | null;
}

/** A busca pelas ferramentas, sem olhar para nenhum servidor */
export interface PgToolsDetection {
  /** A que o app vai usar: a configurada, ou a mais nova achada */
  selected: PgToolsInstall | null;
  /** Todas as achadas, da mais nova para a mais velha */
  candidates: PgToolsInstall[];
  /** A pasta das Preferências não serve */
  configuredError: string | null;
}

/** Por que não dá para fazer backup/restore neste servidor */
export type PgToolsProblem =
  | { kind: 'notFound' }
  | { kind: 'invalidConfigured'; message: string }
  | { kind: 'clientTooOld'; client: string; server: string };

/** As ferramentas frente a um servidor */
export interface PgToolsStatus extends PgToolsDetection {
  /** `server_version` sem o sufixo da distribuição (`16.4`) */
  serverVersion: string;
  serverVersionNum: number;
  problem: PgToolsProblem | null;
}

/** Evento global do Tauri com o andamento dos backups/restaurações */
export const BACKUP_EVENT = 'backup-event';

/** O que entra no backup: o nó da árvore em que o usuário clicou */
export type BackupTarget =
  | { kind: 'database' }
  | { kind: 'schema'; schema: string }
  /** Tabela, view ou sequência (o `pg_dump -t` casa todas) */
  | { kind: 'table'; schema: string; table: string };

/** `custom` = arquivo do pg_dump (.dump); `plain` = script SQL (.sql) */
export type DumpFormat = 'custom' | 'plain';

export type DumpContent = 'all' | 'schemaOnly' | 'dataOnly';

export interface BackupOptions {
  format: DumpFormat;
  content: DumpContent;
  /** Sem dono e sem GRANTs: o arquivo roda em outro servidor */
  noOwner: boolean;
  /** `DROP ... IF EXISTS` antes de cada `CREATE` — só no script SQL */
  clean: boolean;
  /** `INSERT` com nomes de coluna em vez de `COPY` — só no script SQL */
  columnInserts: boolean;
}

export interface BackupRequest {
  serverId: number;
  database: string;
  target: BackupTarget;
  options: BackupOptions;
}

/** O que o diálogo mostra antes de rodar */
export interface BackupPreview {
  /** A linha de comando, sem a senha */
  command: string;
  /** Nome sugerido para o diálogo de salvar */
  fileName: string;
}

/** Andamento de um job, emitido como `BACKUP_EVENT` */
export type BackupEvent = { jobId: string } & (
  /** Linhas novas do log, em lote */
  | { type: 'log'; lines: string[] }
  /** Terminou e o arquivo está no lugar */
  | {
      type: 'done';
      /** O arquivo gerado (backup); vazio na restauração */
      path: string;
      sizeBytes: number;
      elapsedMs: number;
      /** Erros que a ferramenta ignorou e seguiu (restauração sem "tudo ou nada") */
      warnings: string[];
    }
  /** Falhou; nenhum arquivo pela metade ficou */
  | { type: 'error'; message: string }
  | { type: 'cancelled' }
);

/** Formato do arquivo a restaurar, detectado pelo conteúdo */
export type RestoreFormat = 'custom' | 'plain';

/** O que há num `.dump`, pelo `pg_restore --list` */
export interface ArchiveSummary {
  /** Database de onde saiu */
  database: string | null;
  serverVersion: string | null;
  pgDumpVersion: string | null;
  /** Os primeiros schemas criados pelo arquivo */
  schemas: string[];
  schemaCount: number;
  tableCount: number;
  viewCount: number;
  /** Há linhas de dados — senão é só estrutura */
  hasData: boolean;
}

/** O arquivo escolhido para restaurar */
export interface RestoreInspection {
  path: string;
  format: RestoreFormat;
  sizeBytes: number;
  /** Só no `.dump`, quando o pg_restore conseguiu ler */
  summary: ArchiveSummary | null;
  /** Por que não há resumo */
  summaryError: string | null;
}

export interface RestoreOptions {
  /** Ignorar dono e GRANTs do arquivo — só no `.dump` */
  noOwner: boolean;
  /** Apagar os objetos que já existem antes de recriar — só no `.dump` */
  clean: boolean;
  /** Tudo ou nada: uma transação, parando no primeiro erro */
  singleTransaction: boolean;
}

export interface RestoreRequest {
  serverId: number;
  database: string;
  format: RestoreFormat;
  options: RestoreOptions;
}
