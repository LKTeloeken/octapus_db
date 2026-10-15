use serde::{Deserialize, Serialize};

/// Nome do evento global do Tauri com o andamento dos backups/restaurações.
pub const BACKUP_EVENT: &str = "backup-event";

/// De onde veio a instalação das ferramentas do Postgres.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PgToolsSource {
    /// Pasta escolhida nas Preferências
    Configured,
    /// Achada no `PATH` do processo
    Path,
    /// Achada num local de instalação conhecido (Homebrew, Postgres.app, …)
    KnownLocation,
}

/// Uma pasta com o `pg_dump` (e, se houver, `pg_restore` e `psql`).
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PgToolsInstall {
    pub dir: String,
    pub source: PgToolsSource,
    /// Versão como o `pg_dump --version` escreve (`16.4`, `17beta1`)
    pub version: String,
    /// No formato do `server_version_num` (160004 = 16.4), para comparar
    pub version_num: u32,
    pub pg_dump: String,
    pub pg_restore: Option<String>,
    pub psql: Option<String>,
}

/// Resultado da busca pelas ferramentas, sem olhar para nenhum servidor.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PgToolsDetection {
    /// A instalação que o app vai usar (a configurada, ou a mais nova achada)
    pub selected: Option<PgToolsInstall>,
    /// Todas as instalações achadas, da mais nova para a mais velha
    pub candidates: Vec<PgToolsInstall>,
    /// A pasta configurada não serve (não existe, sem `pg_dump`, …)
    pub configured_error: Option<String>,
}

/// Por que não dá para rodar backup/restore neste servidor.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(tag = "kind", rename_all = "camelCase")]
pub enum PgToolsProblem {
    /// Nenhuma instalação encontrada
    NotFound,
    /// A pasta das Preferências não serve
    #[serde(rename_all = "camelCase")]
    InvalidConfigured { message: String },
    /// O `pg_dump` é de uma major anterior à do servidor (ele se recusa a rodar)
    #[serde(rename_all = "camelCase")]
    ClientTooOld { client: String, server: String },
}

/// As ferramentas frente a um servidor: dá para fazer backup dele?
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct PgToolsStatus {
    #[serde(flatten)]
    pub detection: PgToolsDetection,
    /// `server_version` sem o sufixo da distribuição (`16.4`)
    pub server_version: String,
    pub server_version_num: u32,
    pub problem: Option<PgToolsProblem>,
}

/// O que entra no backup: o nó da árvore em que o usuário clicou.
#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(
    tag = "kind",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum BackupTarget {
    Database,
    Schema {
        schema: String,
    },
    /// Tabela, view ou sequência (o `pg_dump -t` casa todas)
    Table {
        schema: String,
        table: String,
    },
}

/// Formato do arquivo gerado.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DumpFormat {
    /// Arquivo do `pg_dump` (`.dump`), lido pelo `pg_restore`
    Custom,
    /// Script SQL (`.sql`), lido pelo `psql`
    Plain,
}

impl DumpFormat {
    pub fn extension(self) -> &'static str {
        match self {
            Self::Custom => "dump",
            Self::Plain => "sql",
        }
    }
}

/// Estrutura, dados ou os dois.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum DumpContent {
    All,
    SchemaOnly,
    DataOnly,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupOptions {
    pub format: DumpFormat,
    pub content: DumpContent,
    /// Sem dono e sem GRANTs (`--no-owner --no-privileges`): o arquivo roda
    /// em outro servidor, com outros usuários
    pub no_owner: bool,
    /// `DROP ... IF EXISTS` antes de cada `CREATE` — só no script SQL (no
    /// `.dump` isso é escolhido na restauração)
    pub clean: bool,
    /// `INSERT` com nomes de coluna em vez de `COPY` — só no script SQL
    pub column_inserts: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupRequest {
    pub server_id: i64,
    pub database: String,
    pub target: BackupTarget,
    pub options: BackupOptions,
}

/// O que o diálogo mostra antes de rodar.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupPreview {
    /// A linha de comando, sem a senha (que vai por variável de ambiente)
    pub command: String,
    /// Nome sugerido para o diálogo de salvar
    pub file_name: String,
}

/// Andamento de um job, emitido como [`BACKUP_EVENT`].
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct BackupEvent {
    pub job_id: String,
    #[serde(flatten)]
    pub kind: BackupEventKind,
}

#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(
    tag = "type",
    rename_all = "camelCase",
    rename_all_fields = "camelCase"
)]
pub enum BackupEventKind {
    /// Linhas novas do log (o `--verbose`), em lote
    Log { lines: Vec<String> },
    /// Terminou e o arquivo está no lugar
    Done {
        /// O arquivo gerado (backup); vazio na restauração
        path: String,
        size_bytes: u64,
        elapsed_ms: u64,
        /// Erros que a ferramenta ignorou e seguiu (restauração sem "tudo ou
        /// nada"); vazio quando foi tudo limpo
        warnings: Vec<String>,
    },
    /// Falhou; nenhum arquivo pela metade ficou para trás
    Error { message: String },
    /// Cancelado a pedido; idem
    Cancelled,
}

/// Formato de um arquivo a restaurar, detectado pelo conteúdo.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum RestoreFormat {
    /// Arquivo do `pg_dump -Fc` (cabeçalho `PGDMP`) → `pg_restore`
    Custom,
    /// Script SQL → `psql`
    Plain,
}

/// O que há num `.dump`, pelo `pg_restore --list`.
#[derive(Debug, Clone, Default, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ArchiveSummary {
    /// Database de onde saiu
    pub database: Option<String>,
    /// Versão do servidor de origem e do `pg_dump` que gerou
    pub server_version: Option<String>,
    pub pg_dump_version: Option<String>,
    /// Schemas criados pelo arquivo (os primeiros, em ordem)
    pub schemas: Vec<String>,
    pub schema_count: usize,
    pub table_count: usize,
    pub view_count: usize,
    /// Há linhas de dados (`TABLE DATA`) — senão é só estrutura
    pub has_data: bool,
}

/// O arquivo escolhido para restaurar.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreInspection {
    pub path: String,
    pub format: RestoreFormat,
    pub size_bytes: u64,
    /// Só no `.dump` e se o `pg_restore` conseguiu ler
    pub summary: Option<ArchiveSummary>,
    /// Por que não há resumo (sem `pg_restore`, arquivo de versão mais nova…)
    pub summary_error: Option<String>,
}

#[derive(Debug, Clone, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreOptions {
    /// Ignorar dono e GRANTs do arquivo — só no `.dump` (no script SQL eles já
    /// estão escritos)
    pub no_owner: bool,
    /// Apagar os objetos que já existem antes de recriar — só no `.dump`
    pub clean: bool,
    /// Tudo ou nada: uma transação, parando no primeiro erro
    pub single_transaction: bool,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct RestoreRequest {
    pub server_id: i64,
    pub database: String,
    pub format: RestoreFormat,
    pub options: RestoreOptions,
}
