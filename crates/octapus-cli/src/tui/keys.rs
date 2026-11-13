//! Os atalhos da TUI: ação → teclas, com os padrões daqui e as trocas do
//! `[keys]` do `config.toml`.
//!
//! Há dois grupos. Os **globais** valem em qualquer painel, inclusive no
//! editor — por isso só aceitam teclas que não escrevem texto (com Ctrl/Alt,
//! teclas de função, Tab, Esc). Os de **navegação** valem na árvore e na
//! grade, onde uma letra sozinha é comando (`j`, `k`, `e`…).

use std::collections::BTreeMap;

use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};

#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash, PartialOrd, Ord)]
pub enum Action {
    // Globais
    Quit,
    NextPane,
    PrevPane,
    FocusTree,
    FocusEditor,
    FocusGrid,
    Run,
    Save,
    Cancel,
    Help,
    Complete,
    Search,
    // Navegação (árvore e grade)
    Up,
    Down,
    Left,
    Right,
    PageUp,
    PageDown,
    Top,
    Bottom,
    Open,
    Filter,
    Refresh,
    Edit,
    SetNull,
    DeleteRow,
    AddRow,
    Undo,
    QuitNav,
    HelpNav,
}

impl Action {
    /// Todas, com o nome do `config.toml`, as teclas padrão e a descrição da
    /// ajuda (F1).
    pub const ALL: &'static [(Action, &'static str, &'static [&'static str], &'static str)] = &[
        (Action::Quit, "quit", &["ctrl+q"], "sair"),
        (Action::NextPane, "next_pane", &["tab"], "próximo painel"),
        (Action::PrevPane, "prev_pane", &["shift+tab"], "painel anterior"),
        (Action::FocusTree, "focus_tree", &["f2"], "ir para a árvore"),
        (Action::FocusEditor, "focus_editor", &["f3"], "ir para o editor"),
        (Action::FocusGrid, "focus_grid", &["f4"], "ir para os resultados"),
        (Action::Run, "run", &["f5", "ctrl+r", "alt+enter"], "executar a consulta"),
        (Action::Save, "save", &["ctrl+s"], "gravar as alterações pendentes"),
        (Action::Cancel, "cancel", &["esc"], "cancelar / fechar"),
        (Action::Help, "help", &["f1"], "esta ajuda"),
        (Action::Complete, "complete", &["ctrl+space", "ctrl+n"], "autocompletar (editor)"),
        (Action::Search, "search", &["ctrl+p"], "buscar tabela ou schema no catálogo"),
        (Action::Up, "up", &["k", "up"], "subir"),
        (Action::Down, "down", &["j", "down"], "descer"),
        (Action::Left, "left", &["h", "left"], "esquerda / recolher"),
        (Action::Right, "right", &["l", "right"], "direita / expandir"),
        (Action::PageUp, "page_up", &["pageup", "ctrl+u"], "página acima"),
        (Action::PageDown, "page_down", &["pagedown", "ctrl+d"], "página abaixo"),
        (Action::Top, "top", &["g", "home"], "início"),
        (Action::Bottom, "bottom", &["G", "end"], "fim"),
        (Action::Open, "open", &["enter"], "abrir / expandir / ver a célula"),
        (Action::Filter, "filter", &["/"], "filtrar a árvore"),
        (Action::Refresh, "refresh", &["r"], "recarregar"),
        (Action::Edit, "edit", &["e"], "editar a célula"),
        (Action::SetNull, "set_null", &["n"], "célula = NULL"),
        (Action::DeleteRow, "delete_row", &["d"], "marcar a linha para excluir"),
        (Action::AddRow, "add_row", &["a"], "nova linha"),
        (Action::Undo, "undo", &["u"], "desfazer a alteração da linha"),
        (Action::QuitNav, "quit_nav", &["q"], "sair"),
        (Action::HelpNav, "help_nav", &["?"], "ajuda"),
    ];

    /// Vale em qualquer painel (inclusive com o cursor no editor)?
    pub fn is_global(self) -> bool {
        matches!(
            self,
            Action::Quit
                | Action::NextPane
                | Action::PrevPane
                | Action::FocusTree
                | Action::FocusEditor
                | Action::FocusGrid
                | Action::Run
                | Action::Save
                | Action::Cancel
                | Action::Help
                | Action::Complete
                | Action::Search
        )
    }
}

/// Uma tecla com os modificadores, já normalizada.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Hash)]
pub struct Combo {
    pub code: KeyCode,
    pub mods: KeyModifiers,
}

impl Combo {
    /// A tecla como o terminal entregou. Numa letra, o Shift já está na
    /// caixa (`G`), então sai dos modificadores; `BackTab` é Shift+Tab.
    pub fn from_event(event: &KeyEvent) -> Self {
        let mut mods = event.modifiers & (KeyModifiers::CONTROL | KeyModifiers::ALT | KeyModifiers::SHIFT);
        let code = match event.code {
            KeyCode::BackTab => {
                mods |= KeyModifiers::SHIFT;
                KeyCode::Tab
            }
            KeyCode::Char(ch) => {
                mods -= KeyModifiers::SHIFT;
                // Ctrl+Espaço chega como NUL em muitos terminais
                match ch {
                    '\0' => {
                        mods |= KeyModifiers::CONTROL;
                        KeyCode::Char(' ')
                    }
                    ch if mods.contains(KeyModifiers::CONTROL) => KeyCode::Char(ch.to_ascii_lowercase()),
                    ch => KeyCode::Char(ch),
                }
            }
            other => other,
        };
        Self { code, mods }
    }

    /// `"ctrl+r"`, `"alt+enter"`, `"f5"`, `"G"`, `"shift+tab"`, `"/"`.
    pub fn parse(text: &str) -> Option<Self> {
        let text = text.trim();
        if text.is_empty() {
            return None;
        }
        let mut mods = KeyModifiers::NONE;
        let mut parts: Vec<&str> = text.split('+').collect();
        // "+" sozinho, ou "ctrl++"
        let key = match parts.pop()? {
            "" => "+",
            key => key,
        };
        for part in parts {
            match part.to_ascii_lowercase().as_str() {
                "ctrl" | "control" => mods |= KeyModifiers::CONTROL,
                "alt" | "option" | "meta" => mods |= KeyModifiers::ALT,
                "shift" => mods |= KeyModifiers::SHIFT,
                "" => {}
                _ => return None,
            }
        }

        let lower = key.to_ascii_lowercase();
        let code = match lower.as_str() {
            "enter" | "return" => KeyCode::Enter,
            "esc" | "escape" => KeyCode::Esc,
            "tab" => KeyCode::Tab,
            "space" => KeyCode::Char(' '),
            "backspace" => KeyCode::Backspace,
            "delete" | "del" => KeyCode::Delete,
            "up" => KeyCode::Up,
            "down" => KeyCode::Down,
            "left" => KeyCode::Left,
            "right" => KeyCode::Right,
            "home" => KeyCode::Home,
            "end" => KeyCode::End,
            "pageup" | "pgup" => KeyCode::PageUp,
            "pagedown" | "pgdn" => KeyCode::PageDown,
            name if name.len() > 1 && name.starts_with('f') => KeyCode::F(name[1..].parse().ok().filter(|n| (1..=12).contains(n))?),
            _ => {
                let mut chars = key.chars();
                let ch = chars.next()?;
                if chars.next().is_some() {
                    return None;
                }
                // Com Shift numa letra, vale a maiúscula; com Ctrl, a minúscula
                let ch = match ch {
                    ch if mods.contains(KeyModifiers::CONTROL) => ch.to_ascii_lowercase(),
                    ch if mods.contains(KeyModifiers::SHIFT) => ch.to_ascii_uppercase(),
                    ch => ch,
                };
                mods -= KeyModifiers::SHIFT;
                KeyCode::Char(ch)
            }
        };
        Some(Self { code, mods })
    }

    /// A tecla escreve texto num campo? (uma letra sem Ctrl nem Alt)
    pub fn types_text(&self) -> bool {
        matches!(self.code, KeyCode::Char(_)) && !self.mods.intersects(KeyModifiers::CONTROL | KeyModifiers::ALT)
    }
}

impl std::fmt::Display for Combo {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        if self.mods.contains(KeyModifiers::CONTROL) {
            f.write_str("Ctrl+")?;
        }
        if self.mods.contains(KeyModifiers::ALT) {
            f.write_str("Alt+")?;
        }
        if self.mods.contains(KeyModifiers::SHIFT) {
            f.write_str("Shift+")?;
        }
        match self.code {
            KeyCode::Char(' ') => f.write_str("Espaço"),
            KeyCode::Char(ch) => write!(f, "{ch}"),
            KeyCode::F(n) => write!(f, "F{n}"),
            KeyCode::Enter => f.write_str("Enter"),
            KeyCode::Esc => f.write_str("Esc"),
            KeyCode::Tab => f.write_str("Tab"),
            KeyCode::Up => f.write_str("↑"),
            KeyCode::Down => f.write_str("↓"),
            KeyCode::Left => f.write_str("←"),
            KeyCode::Right => f.write_str("→"),
            KeyCode::Home => f.write_str("Home"),
            KeyCode::End => f.write_str("End"),
            KeyCode::PageUp => f.write_str("PgUp"),
            KeyCode::PageDown => f.write_str("PgDn"),
            KeyCode::Backspace => f.write_str("Backspace"),
            KeyCode::Delete => f.write_str("Del"),
            other => write!(f, "{other:?}"),
        }
    }
}

pub struct KeyMap {
    bindings: Vec<(Combo, Action)>,
}

impl KeyMap {
    /// Os padrões com as trocas do `[keys]`. Ação que não existe, tecla que
    /// não fecha e atalho global que escreveria texto viram aviso.
    pub fn new(overrides: &BTreeMap<String, Vec<String>>, warnings: &mut Vec<String>) -> Self {
        for name in overrides.keys() {
            if !Action::ALL.iter().any(|(_, known, _, _)| known == name) {
                warnings.push(format!("[keys] ação \"{name}\" não existe"));
            }
        }

        let mut bindings = Vec::new();
        for (action, name, defaults, _) in Action::ALL {
            let keys: Vec<String> = match overrides.get(*name) {
                Some(keys) => keys.clone(),
                None => defaults.iter().map(|key| key.to_string()).collect(),
            };
            for key in keys {
                match Combo::parse(&key) {
                    Some(combo) if action.is_global() && combo.types_text() => warnings.push(format!(
                        "[keys] {name}: \"{key}\" escreveria texto no editor (use Ctrl, Alt ou uma tecla de função)"
                    )),
                    Some(combo) => bindings.push((combo, *action)),
                    None => warnings.push(format!("[keys] {name}: tecla \"{key}\" não reconhecida")),
                }
            }
        }
        Self { bindings }
    }

    /// A ação global da tecla, se houver.
    pub fn global(&self, combo: Combo) -> Option<Action> {
        self.find(combo, true)
    }

    /// A ação de navegação (árvore e grade).
    pub fn nav(&self, combo: Combo) -> Option<Action> {
        self.find(combo, false)
    }

    fn find(&self, combo: Combo, global: bool) -> Option<Action> {
        self.bindings
            .iter()
            .find(|(bound, action)| *bound == combo && action.is_global() == global)
            .map(|(_, action)| *action)
    }

    /// As teclas de uma ação, para a ajuda e as dicas da barra de status.
    pub fn keys_of(&self, action: Action) -> Vec<String> {
        self.bindings
            .iter()
            .filter(|(_, bound)| *bound == action)
            .map(|(combo, _)| combo.to_string())
            .collect()
    }

    pub fn first_key(&self, action: Action) -> String {
        self.keys_of(action).into_iter().next().unwrap_or_else(|| "—".into())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn key(code: KeyCode, mods: KeyModifiers) -> Combo {
        Combo::from_event(&KeyEvent::new(code, mods))
    }

    #[test]
    fn parses_what_the_terminal_sends() {
        assert_eq!(Combo::parse("ctrl+r"), Some(key(KeyCode::Char('r'), KeyModifiers::CONTROL)));
        assert_eq!(Combo::parse("Ctrl+R"), Some(key(KeyCode::Char('r'), KeyModifiers::CONTROL)));
        assert_eq!(Combo::parse("alt+enter"), Some(key(KeyCode::Enter, KeyModifiers::ALT)));
        assert_eq!(Combo::parse("f5"), Some(key(KeyCode::F(5), KeyModifiers::NONE)));
        assert_eq!(Combo::parse("shift+tab"), Some(key(KeyCode::BackTab, KeyModifiers::SHIFT)));
        // A maiúscula chega com Shift; no arquivo é só a letra
        assert_eq!(Combo::parse("G"), Some(key(KeyCode::Char('G'), KeyModifiers::SHIFT)));
        assert_eq!(Combo::parse("shift+g"), Combo::parse("G"));
        assert_ne!(Combo::parse("g"), Combo::parse("G"));
        assert_eq!(Combo::parse("ctrl+space"), Some(key(KeyCode::Char('\0'), KeyModifiers::NONE)));
        assert_eq!(Combo::parse("/"), Some(key(KeyCode::Char('/'), KeyModifiers::NONE)));
        assert_eq!(Combo::parse("+"), Some(key(KeyCode::Char('+'), KeyModifiers::NONE)));
        for bad in ["", "ctrl+", "hyper+x", "f13", "abc"] {
            assert_eq!(Combo::parse(bad).filter(|_| bad != "ctrl+"), None, "{bad}");
        }
    }

    #[test]
    fn defaults_bind_and_overrides_replace() {
        let mut warnings = Vec::new();
        let map = KeyMap::new(&BTreeMap::new(), &mut warnings);
        assert!(warnings.is_empty(), "{warnings:?}");
        assert_eq!(map.global(Combo::parse("f5").unwrap()), Some(Action::Run));
        assert_eq!(map.nav(Combo::parse("j").unwrap()), Some(Action::Down));
        // Uma letra não é atalho global: no editor ela é texto
        assert_eq!(map.global(Combo::parse("j").unwrap()), None);

        let overrides = BTreeMap::from([
            ("run".to_string(), vec!["f9".to_string()]),
            ("quit".to_string(), vec!["x".to_string()]),
            ("fly".to_string(), vec!["f1".to_string()]),
            ("down".to_string(), vec!["nope+j".to_string(), "s".to_string()]),
        ]);
        let map = KeyMap::new(&overrides, &mut warnings);
        assert_eq!(map.global(Combo::parse("f9").unwrap()), Some(Action::Run));
        assert_eq!(map.global(Combo::parse("f5").unwrap()), None);
        assert_eq!(map.nav(Combo::parse("s").unwrap()), Some(Action::Down));
        assert_eq!(warnings.len(), 3, "{warnings:?}");
        assert_eq!(map.first_key(Action::Run), "F9");
    }
}
