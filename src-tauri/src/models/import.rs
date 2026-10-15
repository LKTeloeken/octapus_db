//! Importação de conexões de outros apps (DBeaver, TablePlus, pgAdmin…).
//! Ver `services/connection_import`. Nenhum destes tipos carrega senha: as
//! senhas ficam no backend até o `import_commit`.

use serde::{Deserialize, Serialize};

use super::DatabaseType;

/// App (ou formato) de onde as conexões vêm.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub enum ImportSource {
    Dbeaver,
    Tableplus,
    Pgadmin,
    Beekeeper,
    Datagrip,
    /// URLs coladas (`postgres://…`, `mongodb+srv://…`, `redis://…`)
    Urls,
}

/// Como o app de origem guarda as senhas — decide o que a tela promete.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase")]
pub enum PasswordStorage {
    /// No próprio arquivo (cifrado com chave conhecida): vêm junto
    File,
    /// No Keychain do macOS: o sistema pede permissão para cada senha
    Keychain,
    /// O app não as exporta; só o `~/.pgpass` pode completar (Postgres)
    None,
}

/// Uma origem disponível nesta máquina (ou que aceita arquivo escolhido).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportSourceInfo {
    pub source: ImportSource,
    /// Arquivo encontrado no lugar padrão; `None` = app não encontrado
    pub path: Option<String>,
    /// Conexões no arquivo encontrado (0 sem arquivo)
    pub count: usize,
    pub passwords: PasswordStorage,
    /// Extensões aceitas ao escolher o arquivo à mão (sem o ponto)
    pub file_extensions: Vec<String>,
}

/// Algo que não sai igual do app de origem — a tela explica cada caso.
#[derive(Debug, Clone, PartialEq, Eq, Serialize)]
#[serde(rename_all = "camelCase", tag = "kind")]
pub enum ImportWarning {
    /// A conexão passa por túnel SSH, que o Octapus ainda não suporta
    SshTunnel { host: String },
    /// Banco que o Octapus não abre (MySQL, SQL Server…)
    UnsupportedType { driver: String },
    /// Senha não veio (o app não exporta, ou não estava salva)
    NoPassword,
    /// `verify-ca`/`verify-full`: o Octapus cifra sem verificar o certificado
    SslNotVerified { mode: String },
    /// Certificado de cliente (mTLS), que o Octapus não usa
    ClientCertificate,
    /// Arquivo do SQLite que não existe mais nesta máquina
    FileMissing { path: String },
}

/// Uma conexão lida, já no formato do Octapus (sem a senha).
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCandidate {
    /// Posição na prévia — é por ela que o commit escolhe
    pub key: usize,
    pub name: String,
    /// `None` quando o Octapus não suporta o banco (ver `driver`)
    pub db_type: Option<DatabaseType>,
    /// Nome do banco no app de origem ("PostgreSQL", "MySQL"…)
    pub driver: String,
    pub host: String,
    pub port: u16,
    pub username: String,
    pub default_database: Option<String>,
    pub connection_uri: Option<String>,
    pub ssl_enabled: bool,
    /// Pasta/grupo no app de origem
    pub group: Option<String>,
    /// A senha já está em mãos (arquivo, URL ou `.pgpass`)
    pub has_password: bool,
    /// A senha está no Keychain: só é lida ao importar, se pedido
    pub password_in_keychain: bool,
    /// Servidor do Octapus que parece ser o mesmo (host, porta, usuário, banco)
    pub duplicate_of: Option<i64>,
    pub warnings: Vec<ImportWarning>,
}

/// Resultado da leitura de uma origem. O `token` identifica a prévia no
/// commit — as senhas ficaram no backend, presas a ele.
#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportPreview {
    /// `u32`, não `u64`: o front guarda o token num `Number`, que só é exato
    /// até 2^53. Um `u64` aleatório voltava arredondado e o commit nunca
    /// achava a prévia ("A prévia da importação expirou").
    pub token: u32,
    pub source: ImportSource,
    pub path: Option<String>,
    pub candidates: Vec<ImportCandidate>,
    /// Pastas do app de origem, na ordem em que aparecem
    pub groups: Vec<String>,
}

/// O que importar de uma prévia.
#[derive(Debug, Clone, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportCommitRequest {
    pub token: u32,
    /// `key` dos candidatos escolhidos
    pub keys: Vec<usize>,
    /// Ler as senhas (no TablePlus, do Keychain — o macOS pede permissão)
    pub include_passwords: bool,
    /// Recriar as pastas do app de origem como grupos
    pub keep_groups: bool,
    /// Grupo do Octapus para quem não tem pasta (ou para todos, sem `keep_groups`)
    pub target_group_id: Option<i64>,
}

#[derive(Debug, Clone, Serialize)]
#[serde(rename_all = "camelCase")]
pub struct ImportResult {
    pub imported: usize,
    /// Importadas sem senha (vai ser pedida ao editar o servidor)
    pub without_password: usize,
    /// Leitura do Keychain negada pelo usuário: o resto veio sem senha
    pub keychain_denied: bool,
    pub groups_created: usize,
}
