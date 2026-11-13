//! A configuração da TUI: `~/.config/octapus/config.toml` (ver
//! `octapus paths`). Tudo é opcional — sem arquivo, valem os padrões; um
//! arquivo com erro não impede a TUI de abrir (o erro aparece na barra de
//! status e valem os padrões).
//!
//! ```toml
//! [ui]
//! theme = "dark"      # ou "light"
//! mouse = false       # clique e rolagem (tira a seleção de texto do terminal)
//! mascot = true       # o polvo no painel vazio
//!
//! [theme]             # qualquer token, em #rrggbb
//! accent = "#6660cb"
//!
//! [keys]              # ação = lista de teclas
//! run = ["f5", "ctrl+r"]
//! ```
//!
//! As cores são os tokens do DESIGN.md (Ink) convertidos de OKLCH para sRGB;
//! num terminal sem truecolor elas caem para 256 ou 16 cores, e com
//! `NO_COLOR` a TUI usa só negrito, sublinhado e vídeo reverso.

use std::collections::BTreeMap;
use std::path::Path;

use octapus_core::models::DatabaseType;
use ratatui::style::{Color, Modifier, Style};
use serde::Deserialize;

#[derive(Debug, Default, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct ConfigFile {
    pub ui: UiConfig,
    pub theme: BTreeMap<String, String>,
    pub keys: BTreeMap<String, Vec<String>>,
}

#[derive(Debug, Clone, Deserialize)]
#[serde(default, deny_unknown_fields)]
pub struct UiConfig {
    pub theme: String,
    pub mouse: bool,
    pub mascot: bool,
    /// Avisar quando há versão nova (uma consulta à rede por dia, ao abrir)
    pub update_check: bool,
}

impl Default for UiConfig {
    fn default() -> Self {
        Self {
            theme: "dark".into(),
            mouse: false,
            mascot: true,
            update_check: true,
        }
    }
}

impl ConfigFile {
    /// Lê o arquivo; sem ele, os padrões. `Err` = o arquivo existe e não
    /// fecha (a mensagem vai para a barra de status).
    pub fn load(dir: &Path) -> Result<Self, String> {
        let path = dir.join("config.toml");
        let text = match std::fs::read_to_string(&path) {
            Ok(text) => text,
            Err(error) if error.kind() == std::io::ErrorKind::NotFound => return Ok(Self::default()),
            Err(error) => return Err(format!("{}: {error}", path.display())),
        };
        toml::from_str(&text).map_err(|error| format!("{}: {}", path.display(), error.message()))
    }
}

/// Quantas cores o terminal mostra.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ColorDepth {
    TrueColor,
    Ansi256,
    Ansi16,
    /// `NO_COLOR`: só atributos (negrito, reverso, sublinhado)
    None,
}

impl ColorDepth {
    pub fn detect() -> Self {
        let var = |name: &str| std::env::var(name).unwrap_or_default().to_ascii_lowercase();
        if std::env::var_os("NO_COLOR").is_some_and(|value| !value.is_empty()) {
            return Self::None;
        }
        let colorterm = var("COLORTERM");
        if colorterm.contains("truecolor") || colorterm.contains("24bit") {
            return Self::TrueColor;
        }
        let term = var("TERM");
        if term.contains("256color") || term.contains("kitty") || term.contains("ghostty") {
            return Self::Ansi256;
        }
        Self::Ansi16
    }
}

type Rgb = (u8, u8, u8);

fn parse_hex(value: &str) -> Option<Rgb> {
    let hex = value.trim().strip_prefix('#')?;
    if hex.len() != 6 {
        return None;
    }
    let channel = |range| u8::from_str_radix(hex.get(range)?, 16).ok();
    Some((channel(0..2)?, channel(2..4)?, channel(4..6)?))
}

/// `color` sobre `base`, com a opacidade dos tokens `*-soft` do DESIGN.md.
fn mix(base: Rgb, color: Rgb, alpha: f32) -> Rgb {
    let blend = |a: u8, b: u8| (a as f32 + (b as f32 - a as f32) * alpha).round() as u8;
    (blend(base.0, color.0), blend(base.1, color.1), blend(base.2, color.2))
}

/// O índice mais próximo na paleta de 256 cores (cubo 6x6x6 e cinzas).
fn to_256((r, g, b): Rgb) -> u8 {
    if r == g && g == b {
        return match r {
            0..=7 => 16,
            248..=255 => 231,
            gray => 232 + ((gray as u16 - 8) * 24 / 240) as u8,
        };
    }
    let step = |channel: u8| ((channel as u16 * 5 + 127) / 255) as u8;
    16 + 36 * step(r) + 6 * step(g) + step(b)
}

/// A cor mais próxima entre as 16 do terminal.
fn to_16(rgb: Rgb) -> Color {
    const PALETTE: [(Rgb, Color); 16] = [
        ((0, 0, 0), Color::Black),
        ((170, 0, 0), Color::Red),
        ((0, 170, 0), Color::Green),
        ((170, 85, 0), Color::Yellow),
        ((0, 0, 170), Color::Blue),
        ((170, 0, 170), Color::Magenta),
        ((0, 170, 170), Color::Cyan),
        ((170, 170, 170), Color::Gray),
        ((85, 85, 85), Color::DarkGray),
        ((255, 85, 85), Color::LightRed),
        ((85, 255, 85), Color::LightGreen),
        ((255, 255, 85), Color::LightYellow),
        ((85, 85, 255), Color::LightBlue),
        ((255, 85, 255), Color::LightMagenta),
        ((85, 255, 255), Color::LightCyan),
        ((255, 255, 255), Color::White),
    ];
    let distance = |(r, g, b): Rgb| {
        let delta = |a: u8, b: u8| (a as i32 - b as i32).pow(2);
        delta(r, rgb.0) + delta(g, rgb.1) + delta(b, rgb.2)
    };
    PALETTE
        .iter()
        .min_by_key(|(candidate, _)| distance(*candidate))
        .map_or(Color::Reset, |(_, color)| *color)
}

/// Os tokens do Ink em sRGB (`src/styles/globals.css`, tema escuro e claro).
const DARK: &[(&str, &str)] = &[
    ("bg", "#0d0f11"),
    ("surface", "#141619"),
    ("surface_hi", "#222429"),
    ("line", "#2b2d32"),
    ("fg", "#edeef1"),
    ("muted", "#aeb1b8"),
    ("subtle", "#9b9ea6"),
    ("disabled", "#52555b"),
    ("accent", "#6660cb"),
    ("accent_text", "#a8aaff"),
    ("success", "#54cc8e"),
    ("warning", "#f2b95a"),
    ("danger", "#fd7273"),
    ("info", "#64bced"),
    ("data_number", "#82c7f0"),
    ("data_date", "#7fcfc4"),
    ("data_bool", "#e89dc0"),
    ("data_json", "#eeae7b"),
    ("db_postgres", "#679dd4"),
    ("db_mongo", "#5ac576"),
    ("db_redis", "#f0574e"),
    ("db_sqlite", "#6abfd9"),
];
const LIGHT: &[(&str, &str)] = &[
    ("bg", "#eff0f3"),
    ("surface", "#fdfdfe"),
    ("surface_hi", "#ffffff"),
    ("line", "#d9dade"),
    ("fg", "#181b20"),
    ("muted", "#4f535b"),
    ("subtle", "#5f636c"),
    ("disabled", "#9b9ea5"),
    ("accent", "#5d53cb"),
    ("accent_text", "#584dc5"),
    ("success", "#007845"),
    ("warning", "#945a00"),
    ("danger", "#c2272d"),
    ("info", "#006da8"),
    ("data_number", "#006aa0"),
    ("data_date", "#00736a"),
    ("data_bool", "#a53c75"),
    ("data_json", "#a35303"),
    ("db_postgres", "#316ca5"),
    ("db_mongo", "#1c8742"),
    ("db_redis", "#c9302d"),
    ("db_sqlite", "#0a819d"),
];

/// As cores já convertidas para o que o terminal mostra, e os estilos
/// prontos de cada papel (texto, seleção, pendências da grade…).
#[derive(Debug, Clone)]
pub struct Theme {
    pub depth: ColorDepth,
    rgb: BTreeMap<&'static str, Rgb>,
}

impl Theme {
    /// O tema `name` com as trocas do `[theme]`. Token ou cor que não existe
    /// vira aviso em `warnings`.
    pub fn new(name: &str, overrides: &BTreeMap<String, String>, depth: ColorDepth, warnings: &mut Vec<String>) -> Self {
        let base = match name {
            "light" => LIGHT,
            "dark" => DARK,
            other => {
                warnings.push(format!("tema \"{other}\" não existe (use dark ou light)"));
                DARK
            }
        };
        let mut rgb: BTreeMap<&'static str, Rgb> = base
            .iter()
            .filter_map(|(token, hex)| Some((*token, parse_hex(hex)?)))
            .collect();
        for (token, value) in overrides {
            let known = base.iter().find(|(name, _)| name == token).map(|(name, _)| *name);
            match (known, parse_hex(value)) {
                (Some(name), Some(color)) => {
                    rgb.insert(name, color);
                }
                (None, _) => warnings.push(format!("[theme] token \"{token}\" não existe")),
                (_, None) => warnings.push(format!("[theme] {token}: \"{value}\" não é #rrggbb")),
            }
        }
        Self { depth, rgb }
    }

    fn rgb(&self, token: &str) -> Rgb {
        self.rgb.get(token).copied().unwrap_or((128, 128, 128))
    }

    fn convert(&self, rgb: Rgb) -> Color {
        match self.depth {
            ColorDepth::TrueColor => Color::Rgb(rgb.0, rgb.1, rgb.2),
            ColorDepth::Ansi256 => Color::Indexed(to_256(rgb)),
            ColorDepth::Ansi16 => to_16(rgb),
            ColorDepth::None => Color::Reset,
        }
    }

    pub fn color(&self, token: &str) -> Color {
        self.convert(self.rgb(token))
    }

    /// O fundo `*-soft`: a cor semântica a 20% sobre a superfície.
    fn soft(&self, token: &str) -> Color {
        self.convert(mix(self.rgb("surface"), self.rgb(token), 0.2))
    }

    fn plain(&self, fg: &str, bg: &str) -> Style {
        match self.depth {
            ColorDepth::None => Style::default(),
            _ => Style::default().fg(self.color(fg)).bg(self.color(bg)),
        }
    }

    /// Texto normal sobre a superfície dos painéis.
    pub fn text(&self) -> Style {
        self.plain("fg", "surface")
    }

    pub fn muted(&self) -> Style {
        self.plain("muted", "surface")
    }

    pub fn subtle(&self) -> Style {
        match self.depth {
            ColorDepth::None => Style::default().add_modifier(Modifier::DIM),
            _ => self.plain("subtle", "surface"),
        }
    }

    /// `NULL` e o que está desligado.
    pub fn disabled(&self) -> Style {
        match self.depth {
            ColorDepth::None => Style::default().add_modifier(Modifier::DIM),
            _ => self.plain("disabled", "surface").add_modifier(Modifier::ITALIC),
        }
    }

    /// Cabeçalho de painel: o do painel com foco leva o acento (Iris).
    pub fn header(&self, focused: bool) -> Style {
        match (self.depth, focused) {
            (ColorDepth::None, true) => Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED),
            (ColorDepth::None, false) => Style::default().add_modifier(Modifier::BOLD),
            (_, true) => self.plain("accent_text", "surface_hi").add_modifier(Modifier::BOLD),
            (_, false) => self.plain("muted", "surface_hi"),
        }
    }

    /// Cabeçalho das colunas da grade.
    pub fn column_header(&self) -> Style {
        match self.depth {
            ColorDepth::None => Style::default().add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
            _ => self.plain("muted", "surface_hi").add_modifier(Modifier::BOLD),
        }
    }

    /// A linha selecionada: forte no painel com foco, discreta nos outros.
    pub fn selection(&self, focused: bool) -> Style {
        match (self.depth, focused) {
            (ColorDepth::None, true) => Style::default().add_modifier(Modifier::REVERSED),
            (ColorDepth::None, false) => Style::default().add_modifier(Modifier::UNDERLINED),
            (_, true) => Style::default().fg(self.color("fg")).bg(self.soft_accent()),
            (_, false) => Style::default().fg(self.color("fg")).bg(self.color("surface_hi")),
        }
    }

    fn soft_accent(&self) -> Color {
        self.convert(mix(self.rgb("surface"), self.rgb("accent"), 0.42))
    }

    /// A célula sob o cursor da grade.
    pub fn cell_cursor(&self) -> Style {
        match self.depth {
            ColorDepth::None => Style::default().add_modifier(Modifier::REVERSED | Modifier::BOLD),
            _ => Style::default().fg(self.color("surface")).bg(self.color("accent_text")),
        }
    }

    /// Alterações pendentes da grade, como no app: verde = nova, vermelho =
    /// removida, amarelo = editada.
    pub fn pending(&self, kind: Pending) -> Style {
        let token = match kind {
            Pending::Added => "success",
            Pending::Removed => "danger",
            Pending::Edited => "warning",
        };
        match self.depth {
            ColorDepth::None => match kind {
                Pending::Added => Style::default().add_modifier(Modifier::BOLD),
                Pending::Removed => Style::default().add_modifier(Modifier::CROSSED_OUT),
                Pending::Edited => Style::default().add_modifier(Modifier::UNDERLINED),
            },
            _ => Style::default().fg(self.color("fg")).bg(self.soft(token)),
        }
    }

    /// A cor do valor pelo tipo da coluna (DESIGN.md §2.6): só a tinta, o
    /// fundo é o da linha. Croma baixo: tinge sem gritar.
    pub fn cell(&self, kind: CellKind) -> Style {
        let token = match kind {
            CellKind::Text => "fg",
            CellKind::Number => "data_number",
            CellKind::Bool => "data_bool",
            CellKind::Date => "data_date",
            CellKind::Json => "data_json",
            CellKind::Opaque => "muted",
        };
        match self.depth {
            ColorDepth::None => Style::default(),
            _ => Style::default().fg(self.color(token)),
        }
    }

    /// Uma cor semântica como texto (erro, aviso, sucesso, acento…).
    pub fn tone(&self, token: &str) -> Style {
        match self.depth {
            ColorDepth::None => Style::default().add_modifier(Modifier::BOLD),
            _ => self.plain(token, "surface"),
        }
    }

    /// A barra de status e os divisores.
    pub fn chrome(&self) -> Style {
        self.plain("muted", "bg")
    }

    pub fn divider(&self) -> Style {
        self.plain("line", "surface")
    }

    /// A borda da caixa de um painel: a do painel com foco leva o acento.
    pub fn border(&self, focused: bool) -> Style {
        match (self.depth, focused) {
            (ColorDepth::None, true) => Style::default().add_modifier(Modifier::BOLD),
            (ColorDepth::None, false) => Style::default().add_modifier(Modifier::DIM),
            (_, true) => self.plain("accent_text", "surface"),
            (_, false) => self.plain("line", "surface"),
        }
    }

    /// O título na borda de um painel.
    pub fn title(&self, focused: bool) -> Style {
        match (self.depth, focused) {
            (ColorDepth::None, true) => Style::default().add_modifier(Modifier::BOLD | Modifier::REVERSED),
            (ColorDepth::None, false) => Style::default().add_modifier(Modifier::BOLD),
            (_, true) => self.plain("accent_text", "surface").add_modifier(Modifier::BOLD),
            (_, false) => self.plain("muted", "surface"),
        }
    }

    /// A cor de identidade de um banco (DESIGN.md §2.5): só no ícone.
    pub fn database(&self, db_type: DatabaseType) -> Style {
        self.tone(match db_type {
            DatabaseType::Postgres | DatabaseType::Mysql => "db_postgres",
            DatabaseType::Mongodb => "db_mongo",
            DatabaseType::Redis => "db_redis",
            DatabaseType::Sqlite => "db_sqlite",
        })
    }
}

/// A família do tipo de uma coluna, para a cor e o alinhamento da grade.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum CellKind {
    Text,
    /// Alinhado à direita
    Number,
    /// Mostrado como `true` / `false`
    Bool,
    Date,
    /// JSON, documentos e arrays
    Json,
    /// uuid, binário, ObjectId: identificadores que ninguém lê
    Opaque,
}

impl CellKind {
    /// Pelo nome do tipo que o banco informa (`int4`, `timestamptz`,
    /// `NUMERIC(10,2)`, `_text`, `objectId`…).
    pub fn of(type_name: &str) -> Self {
        let name = type_name.trim().to_ascii_lowercase();
        // `_text` é o typname de `text[]` no Postgres
        if name.starts_with('_') || name.ends_with("[]") {
            return Self::Json;
        }
        let name = name.split('(').next().unwrap_or_default().trim();
        match name {
            "bool" | "boolean" => Self::Bool,
            "json" | "jsonb" | "object" | "array" | "document" | "hstore" => Self::Json,
            "uuid" | "bytea" | "blob" | "binary" | "bindata" | "objectid" | "oid" | "xid" | "cid" | "tid" => Self::Opaque,
            "money" | "number" | "long" | "double" | "real" | "decimal" | "decimal128" | "numeric" => Self::Number,
            name if name.starts_with("date") || name.starts_with("time") || name == "interval" => Self::Date,
            name if is_integer(name) || name.contains("serial") || name.starts_with("float") || name.starts_with("double") => {
                Self::Number
            }
            _ => Self::Text,
        }
    }
}

/// `int`, `integer`, `int4`, `bigint`, `smallint`, `int64`… — mas não
/// `interval` nem `point`.
fn is_integer(name: &str) -> bool {
    let rest = ["big", "small", "tiny", "medium", "u"]
        .iter()
        .find_map(|prefix| name.strip_prefix(prefix))
        .unwrap_or(name);
    rest.strip_prefix("int")
        .is_some_and(|tail| tail.is_empty() || tail == "eger" || tail.bytes().all(|byte| byte.is_ascii_digit()))
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pending {
    Added,
    Removed,
    Edited,
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn colors_fall_back_with_the_terminal() {
        let mut warnings = Vec::new();
        let none = BTreeMap::new();
        let truecolor = Theme::new("dark", &none, ColorDepth::TrueColor, &mut warnings);
        assert_eq!(truecolor.color("accent"), Color::Rgb(0x66, 0x60, 0xcb));
        assert!(matches!(
            Theme::new("dark", &none, ColorDepth::Ansi256, &mut warnings).color("accent"),
            Color::Indexed(_)
        ));
        assert_eq!(Theme::new("dark", &none, ColorDepth::Ansi16, &mut warnings).color("danger"), Color::LightRed);

        // NO_COLOR: nenhuma cor, a seleção vira vídeo reverso
        let plain = Theme::new("dark", &none, ColorDepth::None, &mut warnings);
        assert_eq!(plain.text(), Style::default());
        assert!(plain.selection(true).add_modifier.contains(Modifier::REVERSED));
        assert!(warnings.is_empty());
    }

    #[test]
    fn column_types_fall_into_the_design_families() {
        for (type_name, kind) in [
            ("int4", CellKind::Number),
            ("BIGINT", CellKind::Number),
            ("smallint", CellKind::Number),
            ("int64", CellKind::Number),
            ("NUMERIC(10,2)", CellKind::Number),
            ("double precision", CellKind::Number),
            ("bigserial", CellKind::Number),
            ("bool", CellKind::Bool),
            ("BOOLEAN", CellKind::Bool),
            ("timestamptz", CellKind::Date),
            ("timestamp without time zone", CellKind::Date),
            ("DATE", CellKind::Date),
            ("interval", CellKind::Date),
            ("jsonb", CellKind::Json),
            ("_text", CellKind::Json),
            ("integer[]", CellKind::Json),
            ("uuid", CellKind::Opaque),
            ("objectId", CellKind::Opaque),
            ("varchar(80)", CellKind::Text),
            ("point", CellKind::Text),
            ("", CellKind::Text),
        ] {
            assert_eq!(CellKind::of(type_name), kind, "{type_name}");
        }
    }

    #[test]
    fn overrides_apply_and_bad_ones_become_warnings() {
        let mut warnings = Vec::new();
        let overrides = BTreeMap::from([
            ("accent".to_string(), "#ff0000".to_string()),
            ("nope".to_string(), "#000000".to_string()),
            ("fg".to_string(), "vermelho".to_string()),
        ]);
        let theme = Theme::new("light", &overrides, ColorDepth::TrueColor, &mut warnings);
        assert_eq!(theme.color("accent"), Color::Rgb(255, 0, 0));
        assert_eq!(theme.color("fg"), Color::Rgb(0x18, 0x1b, 0x20));
        assert_eq!(warnings.len(), 2, "{warnings:?}");
    }

    #[test]
    fn config_file_is_optional_and_strict() {
        let dir = std::env::temp_dir().join(format!("octapus-tui-config-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        assert!(ConfigFile::load(&dir).unwrap().ui.mascot);

        std::fs::write(dir.join("config.toml"), "[ui]\nmouse = true\n[keys]\nrun = [\"f9\"]\n").unwrap();
        let config = ConfigFile::load(&dir).unwrap();
        assert!(config.ui.mouse);
        assert_eq!(config.keys["run"], ["f9"]);

        std::fs::write(dir.join("config.toml"), "[ui]\nmouse = \"sim\"\n").unwrap();
        assert!(ConfigFile::load(&dir).is_err());
        let _ = std::fs::remove_dir_all(&dir);
    }
}
