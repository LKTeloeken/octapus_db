//! Importar conexões de outros apps (DBeaver, TablePlus, pgAdmin, Beekeeper,
//! DataGrip) ou de URLs coladas — para quem chega ao Octapus não ter de
//! recadastrar tudo à mão.
//!
//! Fluxo em duas etapas:
//! 1. `preview`: lê a origem, devolve os candidatos **sem senha** e guarda o
//!    lido aqui, preso a um token;
//! 2. `take` + `resolve_passwords` + `store`: com as conexões escolhidas, lê
//!    as senhas (no TablePlus, do Keychain — só agora, e só das escolhidas, já
//!    que cada leitura faz o macOS pedir permissão), cria os grupos e grava os
//!    servidores pelo repositório (que cifra a senha no cofre).
//!
//! Cada app tem seu módulo, que só traduz o formato dele para `RawConnection`;
//! o que vira servidor (e os avisos do que se perde) está em `mapping`.

mod beekeeper;
mod crypto;
mod datagrip;
mod dbeaver;
mod keychain;
mod mapping;
mod paths;
mod pgadmin;
mod sqlite_util;
mod tableplus;
mod url;

use std::collections::HashMap;
use std::path::PathBuf;

use parking_lot::Mutex;
use rusqlite::Connection;

use crate::error::{Error, Result};
use crate::models::{
    ImportCommitRequest, ImportPreview, ImportResult, ImportSource, ImportSourceInfo,
    PasswordStorage,
};
use crate::storage::repositories::{server_groups, servers};

pub use mapping::RawConnection;
use mapping::Secret;

/// O que foi lido na última prévia, à espera do commit. Uma por vez: abrir
/// outra prévia descarta a anterior (e as senhas dela).
#[derive(Default)]
pub struct ConnectionImportService {
    pending: Mutex<Option<Pending>>,
}

struct Pending {
    token: u64,
    raws: Vec<RawConnection>,
}

impl ConnectionImportService {
    /// Monta a prévia de conexões já lidas e as guarda para o commit.
    pub fn preview(
        &self,
        storage: &Mutex<Connection>,
        source: ImportSource,
        path: Option<String>,
        mut raws: Vec<RawConnection>,
    ) -> Result<ImportPreview> {
        // Postgres sem senha: o `.pgpass` é onde o pgAdmin/DataGrip/psql deixam
        let pgpass = paths::pgpass_file()
            .and_then(|file| std::fs::read_to_string(file).ok())
            .map(|text| mapping::parse_pgpass(&text))
            .unwrap_or_default();
        mapping::fill_from_pgpass(&mut raws, &pgpass);

        let existing = servers::get_all(storage)?;
        let candidates = raws
            .iter()
            .enumerate()
            .map(|(key, raw)| mapping::to_candidate(key, raw, &existing))
            .collect();
        let mut groups: Vec<String> = Vec::new();
        for group in raws.iter().filter_map(|raw| raw.group.as_ref()) {
            if !groups.contains(group) {
                groups.push(group.clone());
            }
        }

        let token = rand::random::<u64>();
        *self.pending.lock() = Some(Pending { token, raws });
        Ok(ImportPreview {
            token,
            source,
            path,
            candidates,
            groups,
        })
    }

    /// As conexões escolhidas da prévia (a prévia é consumida). Banco sem
    /// suporte fica de fora mesmo que venha na lista.
    pub fn take(&self, token: u64, keys: &[usize]) -> Result<Vec<RawConnection>> {
        let mut pending = self.pending.lock();
        match pending.as_ref() {
            Some(p) if p.token == token => {}
            _ => {
                return Err(Error::InvalidState(
                    "A prévia da importação expirou — leia as conexões de novo".into(),
                ))
            }
        }
        let Pending { raws, .. } = pending.take().expect("checado acima");
        Ok(raws
            .into_iter()
            .enumerate()
            .filter(|(key, raw)| keys.contains(key) && mapping::db_type_of(&raw.driver).is_some())
            .map(|(_, raw)| raw)
            .collect())
    }

    /// Descarta a prévia (diálogo fechado sem importar).
    pub fn discard(&self) {
        self.pending.lock().take();
    }
}

/// O que há nesta máquina para importar. Lê os arquivos (são pequenos) para
/// contar as conexões; não toca em senha nenhuma.
pub fn detect() -> Vec<ImportSourceInfo> {
    let info =
        |source, files: Vec<PathBuf>, count: usize, passwords, exts: &[&str]| ImportSourceInfo {
            source,
            path: files.first().map(|f| f.to_string_lossy().into_owned()),
            count,
            passwords,
            file_extensions: exts.iter().map(|e| e.to_string()).collect(),
        };

    let dbeaver = dbeaver::find_files();
    let dbeaver_count = dbeaver::read(&dbeaver).map(|r| r.len()).unwrap_or(0);
    let tableplus = tableplus::find_file();
    let tableplus_count = tableplus
        .as_ref()
        .and_then(|f| tableplus::read(f).ok())
        .map_or(0, |r| r.len());
    let pgadmin = paths::pgadmin_db().filter(|f| f.is_file());
    let pgadmin_count = pgadmin
        .as_ref()
        .and_then(|f| pgadmin::read(f).ok())
        .map_or(0, |r| r.len());
    let beekeeper = beekeeper::find_file();
    let beekeeper_count = beekeeper
        .as_ref()
        .and_then(|f| beekeeper::read(f).ok())
        .map_or(0, |r| r.len());
    let datagrip = datagrip::find_files();
    let datagrip_count = datagrip::read(&datagrip).map(|r| r.len()).unwrap_or(0);

    vec![
        info(
            ImportSource::Dbeaver,
            dbeaver,
            dbeaver_count,
            PasswordStorage::File,
            &["json"],
        ),
        info(
            ImportSource::Tableplus,
            tableplus.into_iter().collect(),
            tableplus_count,
            // Só no macOS há Keychain; no Linux a senha vem se estiver no JSON
            if cfg!(target_os = "macos") {
                PasswordStorage::Keychain
            } else {
                PasswordStorage::None
            },
            &["plist", "json"],
        ),
        info(
            ImportSource::Pgadmin,
            pgadmin.into_iter().collect(),
            pgadmin_count,
            PasswordStorage::None,
            &["json", "db"],
        ),
        info(
            ImportSource::Beekeeper,
            beekeeper.into_iter().collect(),
            beekeeper_count,
            PasswordStorage::File,
            &["db"],
        ),
        info(
            ImportSource::Datagrip,
            datagrip,
            datagrip_count,
            PasswordStorage::None,
            &["xml"],
        ),
    ]
}

/// Lê uma origem: do lugar padrão, de um arquivo escolhido (`path`) ou, nas
/// URLs, do texto colado. Devolve o caminho lido, para a tela mostrar.
pub fn read_source(
    source: ImportSource,
    path: Option<String>,
    text: Option<String>,
) -> Result<(Option<String>, Vec<RawConnection>)> {
    let chosen = path.map(PathBuf::from);
    let not_found =
        |app: &str| Error::NotFound(format!("Não encontrei as conexões do {app} nesta máquina"));
    let shown = |files: &[PathBuf]| files.first().map(|f| f.to_string_lossy().into_owned());

    let (files, raws) = match source {
        ImportSource::Urls => {
            let raws: Vec<RawConnection> = text
                .unwrap_or_default()
                .lines()
                .filter_map(url::connection_from_url)
                .collect();
            return Ok((None, raws));
        }
        ImportSource::Dbeaver => {
            let files = chosen.map(|f| vec![f]).unwrap_or_else(dbeaver::find_files);
            if files.is_empty() {
                return Err(not_found("DBeaver"));
            }
            let raws = dbeaver::read(&files)?;
            (files, raws)
        }
        ImportSource::Tableplus => {
            let file = chosen
                .or_else(tableplus::find_file)
                .ok_or_else(|| not_found("TablePlus"))?;
            let raws = tableplus::read(&file)?;
            (vec![file], raws)
        }
        ImportSource::Pgadmin => {
            let file = chosen
                .or_else(|| paths::pgadmin_db().filter(|f| f.is_file()))
                .ok_or_else(|| not_found("pgAdmin"))?;
            let raws = pgadmin::read(&file)?;
            (vec![file], raws)
        }
        ImportSource::Beekeeper => {
            let file = chosen
                .or_else(beekeeper::find_file)
                .ok_or_else(|| not_found("Beekeeper Studio"))?;
            let raws = beekeeper::read(&file)?;
            (vec![file], raws)
        }
        ImportSource::Datagrip => {
            let files = chosen.map(|f| vec![f]).unwrap_or_else(datagrip::find_files);
            if files.is_empty() {
                return Err(not_found("DataGrip"));
            }
            let raws = datagrip::read(&files)?;
            (files, raws)
        }
    };
    Ok((shown(&files), raws))
}

/// A senha de cada conexão, na ordem. Bloqueia: lê o Keychain, onde o macOS
/// pode abrir um aviso de permissão por item. Negou uma vez, para de pedir.
pub fn resolve_passwords(raws: &[RawConnection], include: bool) -> (Vec<String>, bool) {
    let mut denied = false;
    let passwords = raws
        .iter()
        .map(|raw| match &raw.secret {
            _ if !include => String::new(),
            Secret::Plain(password) => password.clone(),
            Secret::Keychain { service, account } if !denied => {
                match keychain::read_password(service, account) {
                    keychain::KeychainRead::Found(password) => password,
                    keychain::KeychainRead::NotFound => String::new(),
                    keychain::KeychainRead::Denied => {
                        denied = true;
                        String::new()
                    }
                }
            }
            _ => String::new(),
        })
        .collect();
    (passwords, denied)
}

/// Cria os grupos e grava os servidores.
pub fn store(
    storage: &Mutex<Connection>,
    raws: &[RawConnection],
    passwords: Vec<String>,
    request: &ImportCommitRequest,
    keychain_denied: bool,
) -> Result<ImportResult> {
    // Pastas do app de origem → grupos (reusa o que já existir com o nome)
    let mut group_ids: HashMap<&str, i64> = HashMap::new();
    let mut groups_created = 0;
    if request.keep_groups {
        let conn = storage.lock();
        let before = count_groups(&conn)?;
        for group in raws.iter().filter_map(|raw| raw.group.as_deref()) {
            if !group_ids.contains_key(group) {
                group_ids.insert(group, server_groups::find_or_create(&conn, group)?);
            }
        }
        groups_created = count_groups(&conn)? - before;
    }

    let mut result = ImportResult {
        imported: 0,
        without_password: 0,
        keychain_denied,
        groups_created,
    };
    for (raw, password) in raws.iter().zip(passwords) {
        let group_id = raw
            .group
            .as_deref()
            .and_then(|group| group_ids.get(group).copied())
            .or(request.target_group_id);
        let has_password = !password.is_empty();
        let Some(input) = mapping::to_server_input(raw, password, group_id) else {
            continue;
        };
        let needs_password = input.db_type != crate::models::DatabaseType::Sqlite
            && input.connection_uri.is_none()
            && !input.username.is_empty();
        servers::create(storage, input)?;
        result.imported += 1;
        if needs_password && !has_password {
            result.without_password += 1;
        }
    }
    Ok(result)
}

fn count_groups(conn: &Connection) -> Result<usize> {
    let count: i64 = conn.query_row("SELECT count(*) FROM server_groups", [], |row| row.get(0))?;
    Ok(count as usize)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::storage::database::init_storage;
    use crate::storage::vault;

    fn storage() -> Mutex<Connection> {
        vault::init_for_tests();
        Mutex::new(init_storage(":memory:").unwrap())
    }

    #[test]
    fn preview_take_and_store_round_trip() {
        let storage = storage();
        let service = ConnectionImportService::default();
        let (_, raws) = read_source(
            ImportSource::Urls,
            None,
            Some(
                "postgres://app:s3cret@db.loja:5432/loja\n\
                 mysql://root@localhost/legado\n\
                 redis://cache.local:6379\n"
                    .into(),
            ),
        )
        .unwrap();
        let mut raws = raws;
        raws[0].group = Some("Produção".into());

        let preview = service
            .preview(&storage, ImportSource::Urls, None, raws)
            .unwrap();
        assert_eq!(preview.candidates.len(), 3);
        assert!(preview.candidates[0].has_password);
        assert_eq!(preview.candidates[1].db_type, None);
        assert_eq!(preview.groups, vec!["Produção".to_string()]);

        // Token errado não consome a prévia
        assert!(service.take(preview.token.wrapping_add(1), &[0]).is_err());

        let request = ImportCommitRequest {
            token: preview.token,
            keys: vec![0, 1, 2],
            include_passwords: true,
            keep_groups: true,
            target_group_id: None,
        };
        let chosen = service.take(request.token, &request.keys).unwrap();
        assert_eq!(chosen.len(), 2, "o MySQL fica de fora");
        let (passwords, denied) = resolve_passwords(&chosen, true);
        let result = store(&storage, &chosen, passwords, &request, denied).unwrap();
        assert_eq!(result.imported, 2);
        assert_eq!(result.groups_created, 1);
        assert_eq!(result.without_password, 0);

        let all = servers::get_all(&storage).unwrap();
        let loja = all.iter().find(|s| s.host == "db.loja").unwrap();
        assert!(loja.group_id.is_some());
        assert_eq!(
            servers::get_by_id(&storage, loja.id.unwrap())
                .unwrap()
                .password,
            "s3cret"
        );

        // A prévia foi consumida
        assert!(service.take(request.token, &request.keys).is_err());

        // De novo: agora é duplicado, e o grupo é reusado
        let (_, again) = read_source(
            ImportSource::Urls,
            None,
            Some("postgres://app@db.loja:5432/loja".into()),
        )
        .unwrap();
        let preview = service
            .preview(&storage, ImportSource::Urls, None, again)
            .unwrap();
        assert_eq!(preview.candidates[0].duplicate_of, loja.id);
    }

    #[test]
    fn without_passwords_flag_drops_them() {
        let raws = vec![RawConnection {
            secret: Secret::Plain("x".into()),
            ..RawConnection::new("a", "PostgreSQL")
        }];
        assert_eq!(resolve_passwords(&raws, false).0, vec![String::new()]);
        assert_eq!(resolve_passwords(&raws, true).0, vec!["x".to_string()]);
    }
}

#[cfg(test)]
mod machine {
    /// Conferência manual: o que há para importar nesta máquina (só contagens).
    /// `cargo test --lib connection_import::machine -- --ignored --nocapture`
    #[test]
    #[ignore]
    fn detect_on_this_machine() {
        for info in super::detect() {
            println!(
                "{:?}: {} conexões em {:?}",
                info.source, info.count, info.path
            );
            let Ok((_, raws)) = super::read_source(info.source, None, None) else {
                continue;
            };
            for (key, raw) in raws.iter().enumerate() {
                let c = super::mapping::to_candidate(key, raw, &[]);
                let warnings: Vec<_> = c
                    .warnings
                    .iter()
                    .map(|w| {
                        format!("{w:?}")
                            .split(' ')
                            .next()
                            .unwrap_or_default()
                            .to_string()
                    })
                    .collect();
                println!(
                    "  {} {:?} porta={} grupo={} senha={} keychain={} {:?}",
                    c.driver,
                    c.db_type,
                    c.port,
                    c.group.is_some(),
                    c.has_password,
                    c.password_in_keychain,
                    warnings
                );
            }
        }
    }
}
