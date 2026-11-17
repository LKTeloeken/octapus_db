//! A pasta de uma rodada. É tudo que a rodada grava, e é só o que a tela lê:
//!
//! | arquivo           | conteúdo                                              |
//! |-------------------|-------------------------------------------------------|
//! | `manifest.json`   | o que torna a rodada reproduzível ([`Manifest`])      |
//! | `gold.json`       | o resultado esperado de cada gabarito                 |
//! | `units.jsonl`     | um teste concluído por linha, só acrescentado         |
//! | `discarded.jsonl` | testes que não abriram ou foram interrompidos         |
//! | `live.json`       | o andamento agora ([`Live`]), regravado por inteiro   |
//! | `results.csv`     | um teste por linha, para a análise                    |
//! | `summary.csv`     | o resumo por paradigma, modelo e idioma               |
//!
//! Um teste só entra no `units.jsonl` depois de terminar, com a linha inteira
//! e o arquivo sincronizado: interromper a rodada não perde o que já foi
//! gravado. A retomada lê as chaves dali. Quando um teste roda de novo
//! (`--redo-failed`), a linha nova vale e a antiga fica no arquivo.

use std::collections::HashMap;
use std::fs::{self, File, OpenOptions};
use std::io::{self, Read, Seek, SeekFrom, Write};
use std::path::{Path, PathBuf};

use parking_lot::Mutex;
use serde::de::DeserializeOwned;
use serde::{Deserialize, Serialize};

use super::config::{AskUser, Backend, ReplyTexts};
use super::questions::Lang;
use super::record::{GoldResult, LiveUnit, UnitRecord};
use crate::models::AssistantModel;

pub const MANIFEST: &str = "manifest.json";
pub const GOLD: &str = "gold.json";
pub const UNITS: &str = "units.jsonl";
pub const DISCARDED: &str = "discarded.jsonl";
pub const LIVE: &str = "live.json";
pub const RESULTS_CSV: &str = "results.csv";
pub const SUMMARY_CSV: &str = "summary.csv";

#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct PromptInfo {
    pub paradigm: String,
    pub version: String,
    /// SHA-256 do texto, o mesmo que a API confere
    pub sha256: String,
}

/// Uma retomada da rodada.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Resume {
    pub at: String,
    pub app_version: String,
    pub commit: Option<String>,
}

/// O que define a rodada e permite repeti-la.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Manifest {
    pub run_id: String,
    pub created_at: String,
    pub app_version: String,
    pub commit: Option<String>,
    /// Havia mudança fora do commit quando a rodada começou
    pub commit_dirty: Option<bool>,
    pub contract_version: String,
    pub api_url: Option<String>,
    pub backend: Backend,
    /// Rodada simulada (`demo`): modelo e registro roteirizados, sem rede
    pub simulated: bool,
    pub prompts: Vec<PromptInfo>,
    pub models: Vec<AssistantModel>,
    pub paradigms: Vec<String>,
    pub languages: Vec<Lang>,
    pub repetitions: u32,
    pub seed: u64,
    pub concurrency: usize,
    pub ask_user: AskUser,
    pub ask_user_reply: Option<ReplyTexts>,
    pub mongo_sample_values: bool,
    pub row_limit: i64,
    /// `--only`: as perguntas a que a rodada foi restrita (vazio = todas)
    pub only: Vec<String>,
    pub questions_version: Option<String>,
    pub questions_sha256: String,
    /// O conjunto de perguntas e o ambiente como foram lidos (o ambiente não
    /// tem segredo: só o nome das variáveis)
    pub questions_toml: String,
    pub environment_toml: String,
    pub total: usize,
    /// As chaves dos testes, na ordem da rodada
    pub order: Vec<String>,
    pub resumes: Vec<Resume>,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "snake_case")]
pub enum RoundStatus {
    Running,
    Finished,
    /// Parada pela linha de comando; `resume` continua
    Interrupted,
    /// Parada sozinha: testes seguidos sem abrir (API fora, cota)
    StoppedOnErrors,
}

/// O andamento da rodada agora.
#[derive(Debug, Clone, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Live {
    pub run_id: String,
    pub status: RoundStatus,
    pub pid: u32,
    /// Quando esta execução (ou retomada) começou
    pub started_at: String,
    /// Regravado ao menos a cada poucos segundos: parado = o processo morreu
    pub updated_at: String,
    pub total: usize,
    pub done: usize,
    pub correct: usize,
    pub incorrect: usize,
    /// Testes que não abriram nesta execução
    pub not_opened: usize,
    pub last_error: Option<String>,
    pub eta_seconds: Option<u64>,
    pub active: Vec<LiveUnit>,
}

pub struct RunDir {
    path: PathBuf,
    units: Mutex<Option<File>>,
    discarded: Mutex<Option<File>>,
}

impl RunDir {
    /// Uma rodada nova: a pasta não pode já ter uma.
    pub fn create(path: &Path) -> io::Result<Self> {
        fs::create_dir_all(path)?;
        // A pasta das rodadas se ignora sozinha no git: os resultados só são
        // versionados se o autor apagar este arquivo
        if let Some(root) = path.parent() {
            let ignore = root.join(".gitignore");
            if !ignore.exists() {
                fs::write(ignore, "# Rodadas do experimento: fora do git por padrão\n*\n")?;
            }
        }
        if path.join(MANIFEST).exists() {
            return Err(io::Error::new(
                io::ErrorKind::AlreadyExists,
                format!("{} já tem uma rodada", path.display()),
            ));
        }
        Ok(Self::at(path))
    }

    /// Uma rodada que já existe.
    pub fn open(path: &Path) -> io::Result<Self> {
        if !path.join(MANIFEST).is_file() {
            return Err(io::Error::new(
                io::ErrorKind::NotFound,
                format!("{} não é a pasta de uma rodada (sem {MANIFEST})", path.display()),
            ));
        }
        Ok(Self::at(path))
    }

    fn at(path: &Path) -> Self {
        Self {
            path: path.to_path_buf(),
            units: Mutex::default(),
            discarded: Mutex::default(),
        }
    }

    pub fn path(&self) -> &Path {
        &self.path
    }

    pub fn write_manifest(&self, manifest: &Manifest) -> io::Result<()> {
        self.write_json(MANIFEST, manifest)
    }

    pub fn manifest(&self) -> io::Result<Manifest> {
        self.read_json(MANIFEST)
    }

    pub fn write_gold(&self, gold: &[GoldResult]) -> io::Result<()> {
        self.write_json(GOLD, &gold)
    }

    pub fn gold(&self) -> io::Result<Vec<GoldResult>> {
        self.read_json(GOLD)
    }

    pub fn write_live(&self, live: &Live) -> io::Result<()> {
        self.write_json(LIVE, live)
    }

    /// Grava o arquivo inteiro de uma vez (temporário + renomear), para a
    /// tela nunca ler pela metade.
    pub fn write_text(&self, name: &str, content: &str) -> io::Result<()> {
        let temporary = self.path.join(format!(".{name}.{}.tmp", std::process::id()));
        fs::write(&temporary, content)?;
        fs::rename(temporary, self.path.join(name))
    }

    fn write_json<T: Serialize + ?Sized>(&self, name: &str, value: &T) -> io::Result<()> {
        self.write_text(name, &serde_json::to_string_pretty(value)?)
    }

    fn read_json<T: DeserializeOwned>(&self, name: &str) -> io::Result<T> {
        let text = fs::read_to_string(self.path.join(name))?;
        serde_json::from_str(&text).map_err(|error| io::Error::new(io::ErrorKind::InvalidData, format!("{name}: {error}")))
    }

    /// Acrescenta um teste concluído e só volta com ele no disco.
    pub fn append_unit(&self, record: &UnitRecord) -> io::Result<()> {
        self.append(&self.units, UNITS, record)
    }

    /// Guarda um teste que não vale como resultado (não abriu, interrompido),
    /// para as conversas que ficaram na API poderem ser identificadas.
    pub fn append_discarded(&self, record: &UnitRecord) -> io::Result<()> {
        self.append(&self.discarded, DISCARDED, record)
    }

    fn append(&self, slot: &Mutex<Option<File>>, name: &str, record: &UnitRecord) -> io::Result<()> {
        let mut slot = slot.lock();
        if slot.is_none() {
            let mut file = OpenOptions::new().create(true).read(true).append(true).open(self.path.join(name))?;
            // Uma linha cortada no fim (processo morto no meio da escrita)
            // não pode grudar na próxima
            let length = file.metadata()?.len();
            if length > 0 {
                let mut last = [0u8; 1];
                file.seek(SeekFrom::Start(length - 1))?;
                file.read_exact(&mut last)?;
                if last != *b"\n" {
                    file.write_all(b"\n")?;
                }
            }
            *slot = Some(file);
        }
        let file = slot.as_mut().expect("aberto acima");
        let mut line = serde_json::to_string(record)?;
        line.push('\n');
        file.write_all(line.as_bytes())?;
        file.sync_data()
    }

    /// Os testes concluídos, na ordem em que entraram; de um teste que rodou
    /// de novo, só o último registro. Linha ilegível (cortada) é pulada.
    pub fn units(&self) -> io::Result<Vec<UnitRecord>> {
        let text = match fs::read_to_string(self.path.join(UNITS)) {
            Ok(text) => text,
            Err(error) if error.kind() == io::ErrorKind::NotFound => return Ok(Vec::new()),
            Err(error) => return Err(error),
        };
        let mut records: Vec<UnitRecord> = Vec::new();
        let mut position: HashMap<String, usize> = HashMap::new();
        for line in text.lines().filter(|line| !line.trim().is_empty()) {
            let Ok(record) = serde_json::from_str::<UnitRecord>(line) else {
                continue;
            };
            match position.get(&record.key) {
                Some(&index) => records[index] = record,
                None => {
                    position.insert(record.key.clone(), records.len());
                    records.push(record);
                }
            }
        }
        Ok(records)
    }
}

/// As pastas de rodada dentro de `root`, da mais nova para a mais antiga.
pub fn list(root: &Path) -> Vec<PathBuf> {
    let Ok(entries) = fs::read_dir(root) else {
        return Vec::new();
    };
    let mut runs: Vec<PathBuf> = entries
        .filter_map(|entry| entry.ok().map(|entry| entry.path()))
        .filter(|path| path.join(MANIFEST).is_file())
        .collect();
    runs.sort();
    runs.reverse();
    runs
}
