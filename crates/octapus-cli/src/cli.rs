//! Os argumentos do `octapus` (clap). Sem subcomando, num terminal, abre a TUI.

use std::path::PathBuf;

use clap::{Args, Parser, Subcommand, ValueEnum};

#[derive(Parser)]
#[command(
    name = "octapus",
    version,
    about = "Octapus no terminal: as mesmas conexões do app, por linha de comando e TUI",
    long_about = "Octapus no terminal: as mesmas conexões do app, por linha de comando e TUI.\n\n\
        Sem subcomando, num terminal, abre a TUI. Os subcomandos servem a scripts e a outros \
        programas: saída estável (--format), erros em stderr e códigos de saída por tipo de erro.",
    after_help = "Códigos de saída: 0 ok · 1 erro · 2 uso · 3 não encontrado · 4 conexão · \
        5 recusado pelo guard de leitura · 6 erro da consulta · 7 não suportado · \
        8 escrita recusada · 130 cancelado"
)]
pub struct Cli {
    /// Formato da saída (padrão: table no terminal, json fora dele)
    #[arg(long, short = 'F', global = true, value_enum)]
    pub format: Option<Format>,

    /// Sem cores (o mesmo que a variável NO_COLOR)
    #[arg(long, global = true)]
    pub no_color: bool,

    #[command(subcommand)]
    pub command: Option<Command>,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Format {
    /// Tabela alinhada, para ler
    Table,
    /// Um documento JSON
    Json,
    /// CSV com cabeçalho (NULL vira campo vazio)
    Csv,
    /// Um objeto JSON por linha
    Ndjson,
}

#[derive(Subcommand)]
pub enum Command {
    /// Conexões salvas (as mesmas do app)
    #[command(subcommand)]
    Server(ServerCommand),

    /// Grupos de servidores
    #[command(subcommand)]
    Group(GroupCommand),

    /// Databases de um servidor
    Dbs(Target),

    /// Schemas (Postgres) ou tabelas de um database
    Ls(LsArgs),

    /// Colunas e índices de uma tabela
    Describe(DescribeArgs),

    /// Linhas de uma tabela, com filtro e ordenação
    Table(TableArgs),

    /// Executa uma consulta. Só leitura, a não ser com --write (que exige um terminal)
    Query(QueryArgs),

    /// Backup de um database, schema ou tabela (PostgreSQL, pelo pg_dump)
    Backup(BackupArgs),

    /// Restaura um arquivo .dump ou .sql. Escreve no banco: exige um terminal
    Restore(RestoreArgs),

    /// Abre a TUI
    Tui(TuiArgs),

    /// Onde ficam os dados, a configuração e o log
    Paths,

    /// Script de autocompletar para o shell
    Completions {
        #[arg(value_enum)]
        shell: clap_complete::Shell,
    },
}

/// O servidor (id ou nome) e, se não for o padrão dele, o database.
#[derive(Args)]
pub struct Target {
    /// Id ou nome do servidor
    pub server: String,

    /// Database (padrão: o do servidor)
    #[arg(long, short = 'd')]
    pub database: Option<String>,
}

#[derive(Subcommand)]
pub enum ServerCommand {
    /// Lista os servidores
    List,
    /// Mostra um servidor (nunca a senha)
    Show { server: String },
    /// Cadastra um servidor, por URL ou por campos
    Add(ServerAddArgs),
    /// Altera campos de um servidor
    Edit(ServerEditArgs),
    /// Exclui um servidor
    Rm {
        server: String,
        /// Não pergunta (obrigatório fora de um terminal)
        #[arg(long, short = 'y')]
        yes: bool,
    },
    /// Testa a conexão
    Test { server: String },
    /// Troca a senha salva (pergunta no terminal, sem eco)
    SetPassword {
        server: String,
        #[command(flatten)]
        password: PasswordSource,
    },
}

/// De onde vem a senha. Nunca do argumento em si: ele aparece no `ps` e no
/// histórico do shell.
#[derive(Args, Default)]
pub struct PasswordSource {
    /// Pergunta a senha no terminal, sem eco
    #[arg(long, short = 'p', conflicts_with_all = ["password_env", "password_stdin"])]
    pub password: bool,

    /// Lê a senha desta variável de ambiente
    #[arg(long, value_name = "VAR", conflicts_with = "password_stdin")]
    pub password_env: Option<String>,

    /// Lê a senha da primeira linha da entrada padrão
    #[arg(long)]
    pub password_stdin: bool,
}

#[derive(Args)]
pub struct ServerAddArgs {
    /// URL de conexão (postgres://, mongodb://, redis://) ou caminho de um arquivo SQLite
    pub url: Option<String>,

    /// Nome na lista (padrão: tirado da URL)
    #[arg(long, short = 'n')]
    pub name: Option<String>,

    /// Tipo do banco, quando não há URL
    #[arg(long = "type", short = 't', value_enum)]
    pub db_type: Option<DbType>,

    #[arg(long, short = 'H')]
    pub host: Option<String>,

    #[arg(long, short = 'P')]
    pub port: Option<u16>,

    #[arg(long, short = 'u')]
    pub user: Option<String>,

    /// Database padrão
    #[arg(long, short = 'd')]
    pub database: Option<String>,

    /// disable, prefer, require, verify-ca ou verify-full
    #[arg(long)]
    pub ssl_mode: Option<String>,

    /// Nome de um grupo existente
    #[arg(long, short = 'g')]
    pub group: Option<String>,

    #[command(flatten)]
    pub password: PasswordSource,
}

#[derive(Args)]
pub struct ServerEditArgs {
    pub server: String,

    #[arg(long, short = 'n')]
    pub name: Option<String>,

    #[arg(long, short = 'H')]
    pub host: Option<String>,

    #[arg(long, short = 'P')]
    pub port: Option<u16>,

    #[arg(long, short = 'u')]
    pub user: Option<String>,

    /// Database padrão
    #[arg(long, short = 'd')]
    pub database: Option<String>,

    /// disable, prefer, require, verify-ca ou verify-full
    #[arg(long)]
    pub ssl_mode: Option<String>,

    /// Nome de um grupo existente; vazio ("") tira do grupo
    #[arg(long, short = 'g')]
    pub group: Option<String>,
}

#[derive(Clone, Copy, Debug, ValueEnum)]
pub enum DbType {
    Postgres,
    Sqlite,
    Mongodb,
    Redis,
}

#[derive(Subcommand)]
pub enum GroupCommand {
    /// Lista os grupos
    List,
    /// Cria um grupo
    Add { name: String },
    /// Renomeia um grupo
    Rename { group: String, name: String },
    /// Exclui um grupo (os servidores ficam fora de grupo)
    Rm { group: String },
}

#[derive(Args)]
pub struct LsArgs {
    #[command(flatten)]
    pub target: Target,

    /// Lista as tabelas deste schema (no Postgres; sem ele, lista os schemas)
    #[arg(long, short = 's')]
    pub schema: Option<String>,
}

#[derive(Args)]
pub struct DescribeArgs {
    #[command(flatten)]
    pub target: Target,

    /// Tabela, ou schema.tabela
    pub table: String,

    #[arg(long, short = 's')]
    pub schema: Option<String>,
}

#[derive(Args)]
pub struct TableArgs {
    #[command(flatten)]
    pub target: Target,

    /// Tabela, ou schema.tabela
    pub table: String,

    #[arg(long, short = 's')]
    pub schema: Option<String>,

    /// Filtro: a condição de um WHERE (SQL), um filtro JSON (Mongo) ou um padrão (Redis)
    #[arg(long, short = 'w', value_name = "EXPR")]
    pub r#where: Option<String>,

    /// Ordenação: coluna ou coluna:desc (pode repetir)
    #[arg(long, short = 'o', value_name = "COLUNA[:desc]")]
    pub order: Vec<String>,

    #[arg(long, short = 'l', default_value_t = 100)]
    pub limit: i64,

    #[arg(long, default_value_t = 0)]
    pub offset: i64,
}

#[derive(Args)]
pub struct QueryArgs {
    #[command(flatten)]
    pub target: Target,

    /// A consulta; sem ela (ou com "-"), lê da entrada padrão
    pub query: Option<String>,

    /// Lê a consulta de um arquivo
    #[arg(long, short = 'f', value_name = "ARQUIVO", conflicts_with = "query")]
    pub file: Option<PathBuf>,

    /// Schema do search_path (Postgres)
    #[arg(long, short = 's')]
    pub schema: Option<String>,

    /// Máximo de linhas
    #[arg(long, short = 'l', default_value_t = 500)]
    pub limit: i64,

    #[arg(long, default_value_t = 0)]
    pub offset: i64,

    /// Permite escrita (INSERT, UPDATE, DDL…). Só num terminal, com confirmação
    #[arg(long)]
    pub write: bool,
}

#[derive(Args)]
pub struct BackupArgs {
    #[command(flatten)]
    pub target: Target,

    /// Só este schema
    #[arg(long, short = 's')]
    pub schema: Option<String>,

    /// Só esta tabela (ou schema.tabela)
    #[arg(long, short = 't')]
    pub table: Option<String>,

    /// Arquivo de saída (padrão: nome com database e data, na pasta atual)
    #[arg(long, short = 'o', value_name = "ARQUIVO")]
    pub output: Option<PathBuf>,

    /// Script SQL em vez do arquivo .dump (também vale um --output terminado em .sql)
    #[arg(long)]
    pub sql: bool,

    /// O que entra: tudo, só a estrutura ou só os dados
    #[arg(long, value_enum, default_value_t = Content::All)]
    pub content: Content,

    /// Sem dono e sem GRANTs
    #[arg(long)]
    pub no_owner: bool,

    /// DROP IF EXISTS antes de cada CREATE (só no script SQL)
    #[arg(long)]
    pub clean: bool,

    /// INSERT com nomes de coluna em vez de COPY (só no script SQL)
    #[arg(long)]
    pub column_inserts: bool,

    /// Sobrescreve o arquivo de saída, se existir
    #[arg(long)]
    pub force: bool,

    /// Só mostra o comando e o arquivo, sem rodar
    #[arg(long)]
    pub dry_run: bool,

    /// Mostra o log do pg_dump no stderr
    #[arg(long, short = 'v')]
    pub verbose: bool,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, ValueEnum)]
pub enum Content {
    All,
    Schema,
    Data,
}

#[derive(Args)]
pub struct RestoreArgs {
    #[command(flatten)]
    pub target: Target,

    /// O arquivo .dump ou .sql
    pub file: PathBuf,

    /// Ignora dono e GRANTs do arquivo (só no .dump)
    #[arg(long)]
    pub no_owner: bool,

    /// Apaga os objetos que já existem antes de recriar (só no .dump)
    #[arg(long)]
    pub clean: bool,

    /// Tudo ou nada: uma transação, parando no primeiro erro
    #[arg(long)]
    pub single_transaction: bool,

    /// Só mostra o arquivo e o comando, sem rodar (não exige terminal)
    #[arg(long)]
    pub dry_run: bool,

    /// Mostra o log da restauração no stderr
    #[arg(long, short = 'v')]
    pub verbose: bool,
}

#[derive(Args, Default)]
pub struct TuiArgs {
    /// Já abre neste servidor (id ou nome)
    pub server: Option<String>,
}
