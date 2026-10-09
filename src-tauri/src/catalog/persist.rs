use std::collections::HashMap;
use std::fs::{self, File};
use std::io::{self, Write};
use std::path::{Path, PathBuf};
use std::time::{Duration, SystemTime};

use serde::{Deserialize, Serialize};

use super::base::Catalog;
use super::interner::Sym;
use super::model::{LoadState, RelKind};
use super::shape::{Fnv, Shape, ShapeId, ShapeTable};
use crate::error::{Error, Result};
use crate::storage::vault;

/// Cabeçalho em claro: identifica o arquivo e a versão antes de decifrar.
const MAGIC: &[u8; 6] = b"OCTCAT";
const HEADER_LEN: usize = MAGIC.len() + 2;

/// Sobe a cada mudança no [`Snapshot`]. Um arquivo de outra versão é
/// descartado e o catálogo volta a ser montado a partir do banco — é cache,
/// não vale migrar.
pub const FORMAT_VERSION: u16 = 2;

const EXTENSION: &str = "cat";
const TMP_EXTENSION: &str = "tmp";

/// Conteúdo do arquivo (depois de decifrar e descomprimir). Os nomes vão uma
/// vez só e o resto aponta para eles por índice, como em memória.
/// v2: `listed` (a camada 0 completa já chegou).
#[derive(Serialize, Deserialize)]
struct Snapshot {
    identity: String,
    fetched_at: Option<i64>,
    listed: bool,
    names: Vec<String>,
    /// Por formato: (nome, tipo, pai) de cada relação, índices em `names`
    shapes: Vec<Vec<(u32, u8, Option<u32>)>>,
    /// (nome, formato, fingerprint, estado)
    schemas: Vec<(String, Option<u32>, Option<i64>, u8)>,
}

/// Por que um arquivo existente foi jogado fora.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Discarded {
    /// Não é um arquivo de catálogo
    BadMagic,
    /// De outra versão do formato
    Version(u16),
    /// Não decifra com a chave deste dispositivo (ou foi adulterado)
    Crypto,
    /// Decifrou, mas o conteúdo não fecha
    Corrupt,
    /// É de outra conexão (host/porta/usuário mudaram)
    Identity,
}

#[derive(Debug)]
pub enum Loaded {
    Hit(Box<Catalog>),
    Miss,
    /// O motivo vai para o diagnóstico; para o serviço, é como um `Miss`
    Discarded(#[allow(dead_code)] Discarded),
}

/// Qual catálogo é este. `identity` deve mudar sempre que o que se enxerga do
/// banco pode mudar (host, porta, usuário): o arquivo antigo deixa de valer.
#[derive(Debug, Clone, Copy)]
pub struct CatalogKey<'a> {
    pub server_id: i64,
    pub database: &'a str,
    pub identity: &'a str,
}

impl CatalogKey<'_> {
    fn file_stem(&self) -> String {
        let mut hasher = Fnv::new();
        hasher.write(self.database.as_bytes());
        format!("{}-{:016x}", self.server_id, hasher.finish())
    }

    /// O que vai dentro do arquivo e é conferido na carga.
    fn full_identity(&self) -> String {
        format!("{}\u{0}{}\u{0}{}", self.server_id, self.database, self.identity)
    }
}

fn io_error(context: &str, error: io::Error) -> Error {
    Error::Storage(format!("{context}: {error}"))
}

impl Catalog {
    fn to_snapshot(&self, identity: &str) -> Snapshot {
        let mut names = Vec::new();
        let mut name_index: HashMap<Sym, u32> = HashMap::new();
        let mut index_of = |sym: Sym, names: &mut Vec<String>| {
            *name_index.entry(sym).or_insert_with(|| {
                names.push(self.names.resolve(sym).to_string());
                (names.len() - 1) as u32
            })
        };

        // Só formatos vivos, renumerados — o arquivo sai compactado
        let mut shape_index: HashMap<ShapeId, u32> = HashMap::new();
        let mut shapes = Vec::new();
        for (id, shape, _) in self.live_shapes() {
            shape_index.insert(id, shapes.len() as u32);
            shapes.push(
                shape
                    .tables
                    .iter()
                    .map(|table| {
                        (
                            index_of(table.name, &mut names),
                            table.kind.code(),
                            table.parent.map(|parent| index_of(parent, &mut names)),
                        )
                    })
                    .collect(),
            );
        }

        let schemas = self
            .schemas
            .iter()
            .map(|(name, entry)| {
                (
                    name.to_string(),
                    entry.shape.map(|shape| shape_index[&shape]),
                    entry.fingerprint,
                    entry.state.code(),
                )
            })
            .collect();

        Snapshot {
            identity: identity.to_string(),
            fetched_at: self.fetched_at(),
            listed: self.knows_schemas(),
            names,
            shapes,
            schemas,
        }
    }

    /// `None` se algum índice ou código não fechar.
    fn from_snapshot(snapshot: Snapshot) -> Option<Catalog> {
        let mut catalog = Catalog::new();
        let syms: Vec<Sym> = snapshot
            .names
            .iter()
            .map(|name| catalog.intern_name(name))
            .collect();
        let sym = |index: u32| syms.get(index as usize).copied();

        let mut shapes = Vec::with_capacity(snapshot.shapes.len());
        for tables in snapshot.shapes {
            let tables = tables
                .into_iter()
                .map(|(name, kind, parent)| {
                    Some(ShapeTable {
                        name: sym(name)?,
                        kind: RelKind::from_code(kind)?,
                        parent: match parent {
                            Some(parent) => Some(sym(parent)?),
                            None => None,
                        },
                    })
                })
                .collect::<Option<Vec<_>>>()?;
            shapes.push(Some(Shape::from_tables(tables, &catalog.names)));
        }

        // Cada formato entra no catálogo na primeira vez que um schema o usa
        let mut interned: Vec<Option<ShapeId>> = vec![None; shapes.len()];
        for (name, shape, fingerprint, state) in snapshot.schemas {
            let state = LoadState::from_code(state)?;
            match shape {
                Some(index) => {
                    let index = index as usize;
                    let id = match interned.get(index)? {
                        Some(id) => *id,
                        None => {
                            let id = catalog.intern_shape(shapes.get_mut(index)?.take()?);
                            interned[index] = Some(id);
                            id
                        }
                    };
                    catalog.assign(&name, id, fingerprint, state);
                }
                None => catalog.insert_unloaded(&name),
            }
        }

        if let Some(at) = snapshot.fetched_at {
            catalog.set_fetched_at(at);
        }
        catalog.set_listed(snapshot.listed);
        Some(catalog)
    }
}

/// Serializa, comprime e cifra: `OCTCAT | versão (u16 LE) | seal(lz4(postcard))`.
pub(super) fn encode(catalog: &Catalog, identity: &str) -> Result<Vec<u8>> {
    let payload = postcard::to_stdvec(&catalog.to_snapshot(identity))
        .map_err(|e| Error::Storage(format!("Failed to serialize catalog: {e}")))?;
    let sealed = vault::seal(&lz4_flex::compress_prepend_size(&payload))?;

    let mut bytes = Vec::with_capacity(HEADER_LEN + sealed.len());
    bytes.extend_from_slice(MAGIC);
    bytes.extend_from_slice(&FORMAT_VERSION.to_le_bytes());
    bytes.extend_from_slice(&sealed);
    Ok(bytes)
}

pub(super) fn decode(bytes: &[u8], identity: &str) -> std::result::Result<Catalog, Discarded> {
    if bytes.len() < HEADER_LEN || &bytes[..MAGIC.len()] != MAGIC {
        return Err(Discarded::BadMagic);
    }
    let version = u16::from_le_bytes([bytes[MAGIC.len()], bytes[MAGIC.len() + 1]]);
    if version != FORMAT_VERSION {
        return Err(Discarded::Version(version));
    }

    // O GCM autentica: daqui em diante o conteúdo foi escrito por este app
    let compressed = vault::open(&bytes[HEADER_LEN..]).ok_or(Discarded::Crypto)?;
    let payload = lz4_flex::decompress_size_prepended(&compressed).map_err(|_| Discarded::Corrupt)?;
    let snapshot: Snapshot = postcard::from_bytes(&payload).map_err(|_| Discarded::Corrupt)?;

    if snapshot.identity != identity {
        return Err(Discarded::Identity);
    }
    Catalog::from_snapshot(snapshot).ok_or(Discarded::Corrupt)
}

/// Os catálogos salvos em disco: um arquivo por (servidor, database), cifrado
/// com a chave do cofre (nomes de tenant costumam ser nomes de clientes).
#[derive(Debug, Clone)]
pub struct CatalogStore {
    dir: PathBuf,
}

impl CatalogStore {
    pub fn new(dir: impl Into<PathBuf>) -> Self {
        Self { dir: dir.into() }
    }

    pub fn path(&self, key: &CatalogKey) -> PathBuf {
        self.dir.join(format!("{}.{EXTENSION}", key.file_stem()))
    }

    /// Grava num temporário e renomeia: quem lê nunca vê um arquivo pela metade.
    pub fn save(&self, key: &CatalogKey, catalog: &Catalog) -> Result<()> {
        fs::create_dir_all(&self.dir).map_err(|e| io_error("Failed to create catalog dir", e))?;
        let bytes = encode(catalog, &key.full_identity())?;

        let path = self.path(key);
        let tmp = path.with_extension(TMP_EXTENSION);
        let mut file = File::create(&tmp).map_err(|e| io_error("Failed to write catalog", e))?;
        file.write_all(&bytes)
            .and_then(|_| file.sync_all())
            .map_err(|e| io_error("Failed to write catalog", e))?;
        drop(file);

        fs::rename(&tmp, &path).map_err(|e| io_error("Failed to replace catalog", e))
    }

    /// Lê o catálogo salvo. Um arquivo inválido (versão, chave, identidade,
    /// conteúdo) é apagado e reportado como [`Loaded::Discarded`] — quem chama
    /// reconstrói do banco.
    pub fn load(&self, key: &CatalogKey) -> Result<Loaded> {
        let path = self.path(key);
        let bytes = match fs::read(&path) {
            Ok(bytes) => bytes,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Loaded::Miss),
            Err(error) => return Err(io_error("Failed to read catalog", error)),
        };

        match decode(&bytes, &key.full_identity()) {
            Ok(catalog) => Ok(Loaded::Hit(Box::new(catalog))),
            Err(reason) => {
                let _ = fs::remove_file(&path);
                Ok(Loaded::Discarded(reason))
            }
        }
    }

    #[cfg(test)]
    pub fn remove(&self, key: &CatalogKey) -> Result<bool> {
        match fs::remove_file(self.path(key)) {
            Ok(()) => Ok(true),
            Err(error) if error.kind() == io::ErrorKind::NotFound => Ok(false),
            Err(error) => Err(io_error("Failed to remove catalog", error)),
        }
    }

    /// Apaga os catálogos de um servidor (servidor excluído do app).
    pub fn purge_server(&self, server_id: i64) -> Result<usize> {
        let prefix = format!("{server_id}-");
        self.purge(|name, _| name.starts_with(&prefix))
    }

    /// Apaga os catálogos não gravados há mais de `max_age` (database que não é
    /// aberto há tempo) e temporários largados por uma gravação interrompida.
    pub fn purge_expired(&self, max_age: Duration) -> Result<usize> {
        let now = SystemTime::now();
        self.purge(|name, modified| {
            let age = modified
                .and_then(|modified| now.duration_since(modified).ok())
                .unwrap_or_default();
            age > max_age || (name.ends_with(TMP_EXTENSION) && age > Duration::from_secs(60))
        })
    }

    fn purge(&self, mut should_remove: impl FnMut(&str, Option<SystemTime>) -> bool) -> Result<usize> {
        let entries = match fs::read_dir(&self.dir) {
            Ok(entries) => entries,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(0),
            Err(error) => return Err(io_error("Failed to list catalogs", error)),
        };

        let mut removed = 0;
        for entry in entries.flatten() {
            let path = entry.path();
            let Some(name) = path.file_name().and_then(|name| name.to_str()) else {
                continue;
            };
            if !is_catalog_file(&path) {
                continue;
            }
            let modified = entry.metadata().and_then(|meta| meta.modified()).ok();
            if should_remove(name, modified) && fs::remove_file(&path).is_ok() {
                removed += 1;
            }
        }
        Ok(removed)
    }
}

fn is_catalog_file(path: &Path) -> bool {
    matches!(
        path.extension().and_then(|ext| ext.to_str()),
        Some(EXTENSION) | Some(TMP_EXTENSION)
    )
}

#[cfg(test)]
pub(super) mod tests {
    use super::*;
    use crate::catalog::model::{CatalogPath, SchemaHeader, TableRecord};

    /// Diretório temporário apagado no fim do teste (sem depender de crate).
    pub struct TempDir(pub PathBuf);

    impl TempDir {
        pub fn new(label: &str) -> Self {
            let unique = format!(
                "octapus-catalog-{label}-{}-{:?}",
                std::process::id(),
                SystemTime::now()
                    .duration_since(SystemTime::UNIX_EPOCH)
                    .unwrap()
                    .as_nanos()
            );
            let dir = std::env::temp_dir().join(unique);
            fs::create_dir_all(&dir).unwrap();
            Self(dir)
        }
    }

    impl Drop for TempDir {
        fn drop(&mut self) {
            let _ = fs::remove_dir_all(&self.0);
        }
    }

    fn sample() -> Catalog {
        let mut catalog = Catalog::new();
        catalog.set_schemas(
            ["public", "tenant_1", "tenant_2", "tenant_3", "events", "empty"]
                .into_iter()
                .map(|name| SchemaHeader {
                    name: name.into(),
                    fingerprint: None,
                }),
        );
        let tenant = || {
            vec![
                TableRecord::new("orders", RelKind::Table),
                TableRecord::new("order_totals", RelKind::MaterializedView),
            ]
        };
        catalog.set_tables("tenant_1", tenant(), Some(11));
        catalog.set_tables("tenant_2", tenant(), Some(12));
        catalog.set_tables("public", vec![TableRecord::new("plans", RelKind::Foreign)], None);
        catalog.set_tables(
            "events",
            vec![
                TableRecord::new("event_log", RelKind::Partitioned),
                TableRecord::partition("event_log_p1", "event_log"),
            ],
            Some(-3),
        );
        catalog.invalidate("tenant_2");
        catalog.set_fetched_at(1_700_000_000_000);
        catalog
    }

    /// Tudo o que se observa de fora — o que a carga precisa reproduzir.
    pub fn observable(catalog: &Catalog) -> String {
        let mut out = format!("{:?} {}\n", catalog.fetched_at(), catalog.knows_schemas());
        let schemas = catalog
            .children(&CatalogPath::Schemas, None, 0, usize::MAX)
            .unwrap();
        out.push_str(&format!("{schemas:?}\n"));
        for schema in &schemas.items {
            let tables = catalog
                .children(&CatalogPath::Schema(schema.name.clone()), None, 0, usize::MAX)
                .unwrap();
            out.push_str(&format!("{tables:?}\n"));
            for table in &tables.items {
                let partitions = catalog.children(
                    &CatalogPath::Partitions {
                        schema: schema.name.clone(),
                        table: table.name.clone(),
                    },
                    None,
                    0,
                    usize::MAX,
                );
                out.push_str(&format!("{partitions:?}\n"));
            }
        }
        out.push_str(&format!("{:?}\n", catalog.pending_schemas()));
        let mut stats = catalog.stats();
        stats.approx_heap_bytes = 0;
        out.push_str(&format!("{stats:?}\n"));
        out
    }

    fn key(database: &str) -> CatalogKey<'_> {
        CatalogKey {
            server_id: 7,
            database,
            identity: "db.example:5432:app",
        }
    }

    #[test]
    fn round_trips_everything_observable() {
        vault::init_for_tests();
        let dir = TempDir::new("roundtrip");
        let store = CatalogStore::new(&dir.0);
        let catalog = sample();

        store.save(&key("app"), &catalog).unwrap();
        let Loaded::Hit(loaded) = store.load(&key("app")).unwrap() else {
            panic!("esperava o catálogo de volta");
        };

        assert_eq!(observable(&loaded), observable(&catalog));
        assert_eq!(loaded.search("ord", 10), catalog.search("ord", 10));
        assert_eq!(loaded.drift(), catalog.drift());
        assert!(!store.path(&key("app")).with_extension(TMP_EXTENSION).exists());
    }

    #[test]
    fn missing_file_is_a_miss() {
        vault::init_for_tests();
        let dir = TempDir::new("miss");
        assert!(matches!(
            CatalogStore::new(&dir.0).load(&key("app")).unwrap(),
            Loaded::Miss
        ));
    }

    /// Grava, deixa `tamper` mexer nos bytes e devolve o resultado da carga.
    fn load_tampered(label: &str, tamper: impl FnOnce(&mut Vec<u8>)) -> (Loaded, bool) {
        vault::init_for_tests();
        let dir = TempDir::new(label);
        let store = CatalogStore::new(&dir.0);
        store.save(&key("app"), &sample()).unwrap();

        let path = store.path(&key("app"));
        let mut bytes = fs::read(&path).unwrap();
        tamper(&mut bytes);
        fs::write(&path, bytes).unwrap();

        let loaded = store.load(&key("app")).unwrap();
        (loaded, path.exists())
    }

    #[test]
    fn other_format_version_is_discarded_and_removed() {
        let (loaded, still_there) = load_tampered("version", |bytes| {
            bytes[MAGIC.len()..HEADER_LEN].copy_from_slice(&(FORMAT_VERSION + 1).to_le_bytes());
        });
        assert!(matches!(loaded, Loaded::Discarded(Discarded::Version(v)) if v == FORMAT_VERSION + 1));
        assert!(!still_there);
    }

    #[test]
    fn tampered_ciphertext_is_discarded() {
        let (loaded, still_there) = load_tampered("crypto", |bytes| {
            let last = bytes.len() - 1;
            bytes[last] ^= 0xff;
        });
        assert!(matches!(loaded, Loaded::Discarded(Discarded::Crypto)));
        assert!(!still_there);
    }

    #[test]
    fn foreign_files_are_discarded() {
        let (loaded, _) = load_tampered("magic", |bytes| bytes[0] = b'X');
        assert!(matches!(loaded, Loaded::Discarded(Discarded::BadMagic)));

        let (loaded, _) = load_tampered("short", |bytes| bytes.truncate(3));
        assert!(matches!(loaded, Loaded::Discarded(Discarded::BadMagic)));
    }

    #[test]
    fn changed_identity_invalidates_the_file() {
        vault::init_for_tests();
        let dir = TempDir::new("identity");
        let store = CatalogStore::new(&dir.0);
        store.save(&key("app"), &sample()).unwrap();

        let moved = CatalogKey {
            identity: "other-host:5432:app",
            ..key("app")
        };
        assert!(matches!(
            store.load(&moved).unwrap(),
            Loaded::Discarded(Discarded::Identity)
        ));
    }

    #[test]
    fn each_database_has_its_own_file_and_servers_purge_alone() {
        vault::init_for_tests();
        let dir = TempDir::new("purge");
        let store = CatalogStore::new(&dir.0);
        let other_server = CatalogKey {
            server_id: 8,
            ..key("app")
        };

        store.save(&key("app"), &sample()).unwrap();
        store.save(&key("billing"), &sample()).unwrap();
        store.save(&other_server, &sample()).unwrap();
        assert_ne!(store.path(&key("app")), store.path(&key("billing")));

        assert_eq!(store.purge_server(7).unwrap(), 2);
        assert!(matches!(store.load(&key("app")).unwrap(), Loaded::Miss));
        assert!(matches!(store.load(&other_server).unwrap(), Loaded::Hit(_)));
        assert!(store.remove(&other_server).unwrap());
        assert!(!store.remove(&other_server).unwrap());
    }

    #[test]
    fn expired_and_abandoned_files_are_purged() {
        vault::init_for_tests();
        let dir = TempDir::new("expire");
        let store = CatalogStore::new(&dir.0);
        store.save(&key("old"), &sample()).unwrap();
        store.save(&key("fresh"), &sample()).unwrap();

        let old = File::options().write(true).open(store.path(&key("old"))).unwrap();
        old.set_modified(SystemTime::now() - Duration::from_secs(40 * 24 * 3600))
            .unwrap();
        let tmp = store.path(&key("crashed")).with_extension(TMP_EXTENSION);
        fs::write(&tmp, b"half").unwrap();
        File::options()
            .write(true)
            .open(&tmp)
            .unwrap()
            .set_modified(SystemTime::now() - Duration::from_secs(300))
            .unwrap();
        fs::write(dir.0.join("unrelated.txt"), b"keep").unwrap();

        assert_eq!(store.purge_expired(Duration::from_secs(30 * 24 * 3600)).unwrap(), 2);
        assert!(matches!(store.load(&key("fresh")).unwrap(), Loaded::Hit(_)));
        assert!(dir.0.join("unrelated.txt").exists());
        assert_eq!(CatalogStore::new(dir.0.join("nope")).purge_server(7).unwrap(), 0);
    }
}
