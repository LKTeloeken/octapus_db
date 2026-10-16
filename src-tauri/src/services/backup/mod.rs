//! Backup e restauração pelas ferramentas nativas do Postgres (`pg_dump`,
//! `pg_restore`, `psql`) — ver PLANO_PG_DUMP_RESTORE.md.
//!
//! Não passa pela trait `DatabaseAdapter`: o trabalho é de um processo
//! externo, não do pool. Aqui moram a localização das ferramentas, a checagem
//! de versão contra o servidor, a montagem da linha de comando e os jobs.

mod args;
#[cfg(test)]
mod e2e;
mod inspect;
mod runner;
mod tools;

pub use args::{
    connection_args, display_command, display_plan, dump_args, needs_timeout_filter, partial_path,
    restore_plan, ssl_env, suggested_file_name, ConnectionArgs, PlanInput, RestorePlan,
};
pub use inspect::{detect_format, summarize};
pub use runner::{BackupService, JobInput, JobSpec};
pub use tools::{detect, status, version_from_server_setting};

use std::path::Path;
use std::process::Stdio;

/// Processo de uma ferramenta do Postgres: sem stdin, morre junto com o
/// handle e, no Windows, sem a janela de console piscando.
pub(crate) fn tool_command(program: &Path) -> tokio::process::Command {
    let mut cmd = tokio::process::Command::new(program);
    cmd.stdin(Stdio::null()).kill_on_drop(true);
    #[cfg(windows)]
    {
        const CREATE_NO_WINDOW: u32 = 0x0800_0000;
        cmd.creation_flags(CREATE_NO_WINDOW);
    }
    cmd
}
