//! O estado da TUI e o que cada tecla faz com ele.
//!
//! Nada aqui espera o banco: toda ida a ele vira uma tarefa em segundo plano
//! que devolve uma [`Msg`]; a tecla seguinte é atendida na hora. Também não
//! há desenho aqui (`view.rs`) nem terminal (`mod.rs`), o que deixa os testes
//! dirigirem a TUI inteira com teclas e mensagens.

use std::future::Future;
use std::sync::Arc;

use octapus_core::adapters::{has_catalog, MessageSink, ReadOnlyOp};
use octapus_core::assistant::{guard, read_only};
use octapus_core::catalog::{CatalogPath, FLAT_SCHEMA};
use octapus_core::models::{
    AdapterCapabilities, CatalogNode, DatabaseType, NodeKind as CatalogKind, QueryMessage, QueryMessageKind,
    QueryOptions, QueryResult, Server, TableDataRequest, TableType,
};
use octapus_core::state::AppState;
use octapus_core::storage::repositories::servers;
use octapus_core::Error;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyEventKind, MouseButton, MouseEvent, MouseEventKind};
use ratatui::layout::Rect;
use tokio::runtime::Handle;
use tokio::sync::mpsc::UnboundedSender;

use crate::context::wait_for_catalog;

use super::config::{Theme, UiConfig};
use super::editor::Editor;
use super::grid::Grid;
use super::keys::{Action, Combo, KeyMap};
use super::tree::{Load, NodeKind, Tree};

/// Linhas por página (a primeira e as seguintes).
const PAGE_ROWS: i64 = 500;
/// Resultados da busca no catálogo (o que cabe numa tela).
const SEARCH_HITS: usize = 40;


#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Pane {
    Tree,
    Editor,
    Grid,
}

/// Onde uma consulta roda, já resolvido (nome do servidor, tipo, database).
#[derive(Debug, Clone, PartialEq)]
pub struct Target {
    pub server_id: i64,
    pub server_name: String,
    pub db_type: DatabaseType,
    pub database: String,
    pub schema: Option<String>,
}

impl Target {
    pub fn label(&self) -> String {
        match &self.schema {
            Some(schema) => format!("{} › {} › {schema}", self.server_name, self.database),
            None => format!("{} › {}", self.server_name, self.database),
        }
    }
}

/// De onde veio o que está na grade — para recarregar e pedir mais páginas.
#[derive(Debug, Clone, PartialEq)]
pub enum Source {
    Table { target: Target, table: String },
    Query { target: Target, text: String, write: bool },
}

impl Source {
    pub fn target(&self) -> &Target {
        match self {
            Source::Table { target, .. } | Source::Query { target, .. } => target,
        }
    }
}

/// O que as tarefas em segundo plano devolvem.
pub enum Msg {
    Children {
        node: usize,
        result: Result<Vec<(NodeKind, String)>, String>,
    },
    Result {
        ticket: u64,
        source: Source,
        result: Result<QueryResult, Error>,
    },
    Page {
        ticket: u64,
        result: Result<QueryResult, String>,
    },
    Saved(Result<u64, String>),
    /// Um catálogo abriu (ou um schema carregou): a busca aberta roda de novo
    CatalogReady(Result<(), String>),
    /// Mensagem do banco durante uma execução (NOTICE, linhas afetadas)
    Notice(String),
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Tone {
    Info,
    Ok,
    Warn,
    Error,
}

#[derive(Debug, Clone, PartialEq)]
pub enum InputPurpose {
    Filter,
    EditCell { row: usize, col: usize },
}

#[derive(Debug, Clone, PartialEq)]
pub enum ConfirmAction {
    RunWrite { target: Target, text: String },
    Quit,
}

#[derive(Debug, Clone, PartialEq)]
pub enum Modal {
    Help,
    /// O valor inteiro de uma célula
    Cell { title: String, text: String },
    Input {
        title: String,
        value: String,
        purpose: InputPurpose,
    },
    Confirm { lines: Vec<String>, action: ConfirmAction },
    /// A busca no catálogo dos databases abertos
    Search {
        value: String,
        hits: Vec<SearchRow>,
        selected: usize,
    },
}

/// Um resultado da busca no catálogo.
#[derive(Debug, Clone, PartialEq)]
pub struct SearchRow {
    pub server_id: i64,
    pub database: String,
    pub name: String,
    pub kind: CatalogKind,
    /// O schema da relação (`None` num schema, num banco sem schemas ou
    /// quando o nome existe em vários)
    pub schema: Option<String>,
    /// Em quantos schemas o nome existe, quando é mais de um
    pub many: Option<usize>,
}

/// Onde cada painel foi desenhado no último quadro (para o mouse e a rolagem).
#[derive(Debug, Default, Clone, Copy)]
pub struct Areas {
    pub tree: Rect,
    pub editor: Rect,
    pub grid: Rect,
}

pub struct Tui {
    pub state: Arc<AppState>,
    pub theme: Theme,
    pub keys: KeyMap,
    pub ui: UiConfig,
    pub servers: Vec<Server>,

    pub tree: Tree,
    pub editor: Editor,
    pub grid: Option<Grid>,
    pub source: Option<Source>,
    pub focus: Pane,
    pub modal: Option<Modal>,
    pub status: Option<(Tone, String)>,
    pub areas: Areas,

    /// Tarefas em andamento (o indicador de atividade)
    pub busy: usize,
    /// A execução atual: respostas de uma anterior são descartadas
    ticket: u64,
    loading_more: bool,
    /// Fase da animação (o polvo e o indicador de atividade)
    pub phase: f32,
    pub quit: bool,

    tx: UnboundedSender<Msg>,
    runtime: Handle,
}

/// As mensagens do banco durante uma escrita, para a barra de status.
struct NoticeSink(UnboundedSender<Msg>);

impl MessageSink for NoticeSink {
    fn push(&self, message: QueryMessage) {
        if matches!(message.kind, QueryMessageKind::Status | QueryMessageKind::Notice | QueryMessageKind::Warning) {
            let _ = self.0.send(Msg::Notice(message.message));
        }
    }
}

fn capabilities(db_type: DatabaseType) -> Result<AdapterCapabilities, Error> {
    AdapterCapabilities::for_db_type(db_type)
        .ok_or_else(|| Error::UnsupportedDatabase(format!("no adapter available for {db_type:?}")))
}

fn table_type(kind: CatalogKind) -> TableType {
    match kind {
        CatalogKind::View => TableType::View,
        CatalogKind::MaterializedView => TableType::MaterializedView,
        CatalogKind::Foreign => TableType::Foreign,
        // A tabela particionada abre como uma tabela (as partições ficam nela)
        CatalogKind::Table | CatalogKind::Partitioned | CatalogKind::Schema => TableType::Table,
    }
}

/// As relações de um schema, do catálogo (carregando o schema se preciso).
async fn catalog_tables(
    state: &AppState,
    server_id: i64,
    database: &str,
    schema: Option<&str>,
    refresh: bool,
) -> Result<Vec<(NodeKind, String)>, Error> {
    let entry = state.catalog_entry(server_id, database).await?;
    let name = schema.unwrap_or(FLAT_SCHEMA).to_string();
    match refresh {
        true => state.catalog.refresh_schema(&entry, &name).await?,
        false => state.catalog.ensure_schemas(&entry, std::slice::from_ref(&name)).await?,
    }
    let page = entry.read().children(&CatalogPath::Schema(name), None, 0, usize::MAX);
    let relations: Vec<CatalogNode> = page.map(|page| page.items).unwrap_or_default();
    Ok(relations
        .into_iter()
        .map(|relation| {
            (
                NodeKind::Table {
                    server_id,
                    database: database.to_string(),
                    schema: schema.map(str::to_string),
                    name: relation.name.clone(),
                    table_type: table_type(relation.kind),
                },
                relation.name,
            )
        })
        .collect())
}

/// Os filhos de um nó da árvore, um nível só. Onde há catálogo (Postgres,
/// Mongo, SQLite) a estrutura vem dele, como no app: da memória ou do disco,
/// com o banco consultado só no que falta. `refresh` relê do banco.
async fn load_children(state: Arc<AppState>, kind: NodeKind, refresh: bool) -> Result<Vec<(NodeKind, String)>, Error> {
    match kind {
        NodeKind::Server { id, .. } => {
            let database = state.default_database(id)?;
            let adapter = state.adapter(id, &database).await?;
            let databases = adapter.list_databases().await?;
            Ok(databases
                .into_iter()
                .map(|database| {
                    (
                        NodeKind::Database {
                            server_id: id,
                            name: database.name.clone(),
                        },
                        database.name,
                    )
                })
                .collect())
        }
        NodeKind::Database { server_id, name } => {
            let db_type = servers::get_by_id_meta(&state.storage, server_id)?.db_type;
            let has_schemas = capabilities(db_type)?.has_schemas;
            if has_catalog(db_type) && !has_schemas {
                return catalog_tables(&state, server_id, &name, None, refresh).await;
            }
            if has_catalog(db_type) {
                let entry = state.catalog_entry(server_id, &name).await?;
                // Quem acabou de abrir já está sincronizando: `false` não é erro
                let forced = refresh && state.catalog.ensure_fresh(&entry, true);
                wait_for_catalog(&state, &entry, forced).await?;
                let page = entry.read().children(&CatalogPath::Schemas, None, 0, usize::MAX);
                return Ok(page
                    .map(|page| page.items)
                    .unwrap_or_default()
                    .into_iter()
                    .map(|schema| {
                        (
                            NodeKind::Schema {
                                server_id,
                                database: name.clone(),
                                name: schema.name.clone(),
                            },
                            schema.name,
                        )
                    })
                    .collect());
            }

            // Sem catálogo (Redis): direto do adapter
            let adapter = state.adapter(server_id, &name).await?;
            let tables = adapter.list_tables("").await?;
            Ok(tables
                .into_iter()
                .map(|table| {
                    (
                        NodeKind::Table {
                            server_id,
                            database: name.clone(),
                            schema: None,
                            name: table.name.clone(),
                            table_type: table.table_type,
                        },
                        table.name,
                    )
                })
                .collect())
        }
        NodeKind::Schema {
            server_id,
            database,
            name,
        } => catalog_tables(&state, server_id, &database, Some(&name), refresh).await,
        NodeKind::Table { .. } => Ok(Vec::new()),
    }
}

/// Uma página do que está na grade, a partir de `offset`.
async fn fetch(state: Arc<AppState>, source: Source, offset: i64, sink: Option<Arc<dyn MessageSink>>) -> Result<QueryResult, Error> {
    let target = source.target().clone();
    let adapter = state.adapter(target.server_id, &target.database).await?;
    match source {
        Source::Table { table, .. } => {
            adapter
                .fetch_table_data(TableDataRequest {
                    schema: target.schema,
                    table,
                    where_expr: None,
                    sort: Vec::new(),
                    limit: PAGE_ROWS,
                    offset,
                    count_total: false,
                    unlimited: false,
                })
                .await
        }
        Source::Query { text, write, .. } => {
            let options = QueryOptions {
                limit: PAGE_ROWS,
                offset,
                schema: target.schema,
                ..QueryOptions::default()
            };
            if write {
                // O caminho do editor do app: a pessoa já confirmou
                return adapter.execute_query_with_messages(&text, options, sink).await;
            }
            let op = ReadOnlyOp::new(capabilities(target.db_type)?.query_language, text);
            read_only::execute(adapter.as_ref(), target.db_type, &op, options).await
        }
    }
}

impl Tui {
    pub fn new(
        state: Arc<AppState>,
        theme: Theme,
        keys: KeyMap,
        ui: UiConfig,
        tx: UnboundedSender<Msg>,
        runtime: Handle,
    ) -> Self {
        let mut tui = Self {
            state,
            theme,
            keys,
            ui,
            servers: Vec::new(),
            tree: Tree::default(),
            editor: Editor::default(),
            grid: None,
            source: None,
            focus: Pane::Tree,
            modal: None,
            status: None,
            areas: Areas::default(),
            busy: 0,
            ticket: 0,
            loading_more: false,
            phase: 0.0,
            quit: false,
            tx,
            runtime,
        };
        tui.load_servers();
        tui
    }

    fn load_servers(&mut self) {
        match servers::get_all(&self.state.storage) {
            Ok(servers) => {
                self.tree.set_roots(
                    servers
                        .iter()
                        .filter_map(|server| {
                            Some((
                                NodeKind::Server {
                                    id: server.id?,
                                    db_type: server.db_type,
                                },
                                server.name.clone(),
                            ))
                        })
                        .collect(),
                );
                self.servers = servers;
            }
            Err(error) => self.set_status(Tone::Error, error.to_string()),
        }
    }

    /// Leva a seleção ao servidor `reference` (id ou nome) e o abre.
    pub fn select_server(&mut self, reference: &str) {
        let wanted = reference.trim();
        let row = (0..self.tree.len()).find(|row| {
            self.tree.at(*row).is_some_and(|(_, node)| match &node.kind {
                NodeKind::Server { id, .. } => id.to_string() == wanted || node.label.eq_ignore_ascii_case(wanted),
                _ => false,
            })
        });
        match row {
            Some(row) => {
                self.tree.select(row);
                self.open_selected();
            }
            None => self.set_status(Tone::Warn, format!("servidor \"{wanted}\" não encontrado")),
        }
    }

    pub fn set_status(&mut self, tone: Tone, message: impl Into<String>) {
        self.status = Some((tone, message.into()));
    }

    /// Há algo se mexendo na tela? Só então o relógio da animação roda.
    pub fn animating(&self) -> bool {
        self.busy > 0 || (self.ui.mascot && self.grid.is_none() && self.modal.is_none())
    }

    pub fn tick(&mut self) {
        self.phase = (self.phase + 0.35) % (std::f32::consts::TAU * 20.0);
    }

    fn spawn(&mut self, task: impl Future<Output = Msg> + Send + 'static) {
        self.busy += 1;
        let tx = self.tx.clone();
        self.runtime.spawn(async move {
            let _ = tx.send(task.await);
        });
    }

    // ── Alvo da consulta ────────────────────────────────────────────────────

    /// Onde a consulta do editor roda: o nó selecionado da árvore; sem nada
    /// selecionado, onde a grade atual veio.
    pub fn target(&self) -> Option<Target> {
        let Some((_, node)) = self.tree.selected_node() else {
            return self.source.as_ref().map(|source| source.target().clone());
        };
        let context = node.kind.context();
        let server = self.servers.iter().find(|server| server.id == Some(context.server_id))?;
        let database = match context.database {
            Some(database) => database,
            None => self.state.default_database(context.server_id).ok()?,
        };
        Some(Target {
            server_id: context.server_id,
            server_name: server.name.clone(),
            db_type: server.db_type,
            database,
            schema: context.schema,
        })
    }

    // ── Teclado ─────────────────────────────────────────────────────────────

    pub fn on_key(&mut self, event: KeyEvent) {
        if event.kind == KeyEventKind::Release {
            return;
        }
        let combo = Combo::from_event(&event);

        if self.modal.is_some() {
            self.modal_key(combo);
            return;
        }
        if self.focus == Pane::Editor && self.editor.completion.is_some() && self.completion_key(combo) {
            return;
        }
        if let Some(action) = self.keys.global(combo) {
            self.global(action);
            return;
        }
        match self.focus {
            Pane::Editor => self.editor_key(combo),
            Pane::Tree => {
                if let Some(action) = self.keys.nav(combo) {
                    self.tree_action(action);
                }
            }
            Pane::Grid => {
                if let Some(action) = self.keys.nav(combo) {
                    self.grid_action(action);
                }
            }
        }
    }

    pub fn on_paste(&mut self, text: &str) {
        match &mut self.modal {
            Some(Modal::Input { value, .. }) => {
                value.push_str(&text.replace(['\n', '\r'], " "));
                self.input_changed();
            }
            Some(Modal::Search { value, .. }) => {
                value.push_str(&text.replace(['\n', '\r'], " "));
                self.search_changed();
            }
            Some(_) => {}
            None if self.focus == Pane::Editor => self.editor.insert_str(text),
            None => {}
        }
    }

    fn global(&mut self, action: Action) {
        match action {
            Action::Quit => self.request_quit(),
            Action::NextPane => {
                self.focus = match self.focus {
                    Pane::Tree => Pane::Editor,
                    Pane::Editor => Pane::Grid,
                    Pane::Grid => Pane::Tree,
                }
            }
            Action::PrevPane => {
                self.focus = match self.focus {
                    Pane::Tree => Pane::Grid,
                    Pane::Editor => Pane::Tree,
                    Pane::Grid => Pane::Editor,
                }
            }
            Action::FocusTree => self.focus = Pane::Tree,
            Action::FocusEditor => self.focus = Pane::Editor,
            Action::FocusGrid => self.focus = Pane::Grid,
            Action::Run => self.run_editor(),
            Action::Save => self.save(),
            Action::Cancel => {
                if !self.tree.filter().is_empty() && self.focus == Pane::Tree {
                    self.tree.set_filter("");
                }
                self.status = None;
            }
            Action::Help => self.modal = Some(Modal::Help),
            Action::Search => self.open_search(),
            Action::Complete if self.focus == Pane::Editor => {
                let names = self.completion_names();
                self.editor.complete(&names);
            }
            _ => {}
        }
    }

    fn request_quit(&mut self) {
        match self.grid.as_ref().map_or(0, Grid::pending_count) {
            0 => self.quit = true,
            pending => {
                self.modal = Some(Modal::Confirm {
                    lines: vec![format!("Há {pending} alteração(ões) pendente(s) que não foram gravadas."), "Sair mesmo assim?".into()],
                    action: ConfirmAction::Quit,
                })
            }
        }
    }

    /// Os nomes que o autocompletar oferece: as tabelas já vistas na árvore e
    /// as colunas da grade.
    fn completion_names(&self) -> Vec<String> {
        let mut names: Vec<String> = Vec::new();
        if let Some(grid) = &self.grid {
            names.extend(grid.columns.iter().cloned());
        }
        for row in 0..self.tree.len() {
            if let Some((_, node)) = self.tree.at(row) {
                if matches!(node.kind, NodeKind::Table { .. } | NodeKind::Schema { .. }) {
                    names.push(node.label.clone());
                }
            }
        }
        names
    }

    /// O autocompletar aberto come as teclas dele; `false` = a tecla segue.
    fn completion_key(&mut self, combo: Combo) -> bool {
        match combo.code {
            KeyCode::Up => self.editor.move_completion(-1),
            KeyCode::Down => self.editor.move_completion(1),
            KeyCode::Enter | KeyCode::Tab => self.editor.accept_completion(),
            KeyCode::Esc => self.editor.completion = None,
            _ => {
                if self.keys.global(combo) == Some(Action::Complete) {
                    self.editor.move_completion(1);
                    return true;
                }
                self.editor.completion = None;
                return false;
            }
        }
        true
    }

    fn editor_key(&mut self, combo: Combo) {
        match combo.code {
            KeyCode::Char(ch) if combo.types_text() => self.editor.insert_char(ch),
            KeyCode::Enter => self.editor.newline(),
            KeyCode::Backspace => self.editor.backspace(),
            KeyCode::Delete => self.editor.delete(),
            KeyCode::Left => self.editor.move_left(),
            KeyCode::Right => self.editor.move_right(),
            KeyCode::Up => self.editor.move_vertical(-1),
            KeyCode::Down => self.editor.move_vertical(1),
            KeyCode::PageUp => self.editor.move_vertical(-(self.areas.editor.height as isize)),
            KeyCode::PageDown => self.editor.move_vertical(self.areas.editor.height as isize),
            KeyCode::Home => self.editor.move_home(),
            KeyCode::End => self.editor.move_end(),
            _ => {}
        }
    }

    // ── Árvore ──────────────────────────────────────────────────────────────

    fn tree_action(&mut self, action: Action) {
        let page = self.areas.tree.height.saturating_sub(1).max(1) as isize;
        match action {
            Action::Up => self.tree.move_by(-1),
            Action::Down => self.tree.move_by(1),
            Action::PageUp => self.tree.move_by(-page),
            Action::PageDown => self.tree.move_by(page),
            Action::Top => self.tree.select(0),
            Action::Bottom => self.tree.select(usize::MAX),
            Action::Open => self.open_selected(),
            Action::Right => match self.tree.selected_node().map(|(id, node)| (id, node.expanded, node.kind.is_leaf())) {
                Some((_, true, _)) => self.tree.move_by(1),
                Some((_, false, false)) => self.open_selected(),
                _ => {}
            },
            Action::Left => match self.tree.selected_node().map(|(id, node)| (id, node.expanded)) {
                Some((id, true)) => self.tree.set_expanded(id, false),
                Some((_, false)) => {
                    if let Some(parent) = self.tree.parent_row(self.tree.selected) {
                        self.tree.select(parent);
                    }
                }
                None => {}
            },
            Action::Filter => {
                self.modal = Some(Modal::Input {
                    title: "Filtrar a árvore".into(),
                    value: self.tree.filter().to_string(),
                    purpose: InputPurpose::Filter,
                })
            }
            Action::Refresh => {
                if let Some((id, false)) = self.tree.selected_node().map(|(id, node)| (id, node.kind.is_leaf())) {
                    self.tree.reset(id);
                    self.load_selected(true);
                }
            }
            Action::QuitNav => self.request_quit(),
            Action::HelpNav => self.modal = Some(Modal::Help),
            _ => {}
        }
    }

    /// Enter num nó: abre a tabela na grade, ou expande/recolhe o resto.
    fn open_selected(&mut self) {
        let Some((id, node)) = self.tree.selected_node() else {
            return;
        };
        let (kind, expanded, load) = (node.kind.clone(), node.expanded, node.load);

        if let NodeKind::Table { name, .. } = &kind {
            if let Some(target) = self.target() {
                self.execute(Source::Table {
                    target,
                    table: name.clone(),
                });
                self.focus = Pane::Grid;
            }
            return;
        }
        match load {
            Load::Loaded => self.tree.set_expanded(id, !expanded),
            Load::Loading => {}
            Load::Idle | Load::Failed => self.load_selected(false),
        }
    }

    /// Busca os filhos do nó selecionado; `refresh` relê a estrutura do banco
    /// em vez de usar o que o catálogo já tem.
    fn load_selected(&mut self, refresh: bool) {
        let Some((id, kind)) = self.tree.selected_node().map(|(id, node)| (id, node.kind.clone())) else {
            return;
        };
        if self.tree.begin_load(id) {
            let state = Arc::clone(&self.state);
            self.spawn(async move {
                Msg::Children {
                    node: id,
                    result: load_children(state, kind, refresh).await.map_err(|error| error.to_string()),
                }
            });
        }
    }

    // ── Busca no catálogo ───────────────────────────────────────────────────

    /// Abre a busca. Ela olha os catálogos já abertos; o do nó selecionado é
    /// aberto agora, se ainda não estiver.
    fn open_search(&mut self) {
        self.modal = Some(Modal::Search {
            value: String::new(),
            hits: Vec::new(),
            selected: 0,
        });
        let Some(target) = self.target() else {
            return;
        };
        if !has_catalog(target.db_type) || self.state.catalog.get(target.server_id, &target.database).is_some() {
            return;
        }
        let state = Arc::clone(&self.state);
        self.spawn(async move {
            let opened = async {
                let entry = state.catalog_entry(target.server_id, &target.database).await?;
                wait_for_catalog(&state, &entry, false).await
            };
            Msg::CatalogReady(opened.await.map_err(|error| error.to_string()))
        });
    }

    /// Refaz os resultados da busca aberta (da memória: não vai ao banco).
    fn search_changed(&mut self) {
        let Some(Modal::Search { value, .. }) = &self.modal else {
            return;
        };
        let found: Vec<SearchRow> = self
            .state
            .catalog
            .search_all(value, SEARCH_HITS)
            .into_iter()
            .map(|found| SearchRow {
                server_id: found.server_id,
                database: found.database,
                name: found.hit.name,
                kind: found.hit.kind,
                schema: found.hit.schema.filter(|schema| schema != FLAT_SCHEMA),
                many: found.hit.schemas.map(|group| group.total).filter(|total| *total > 1),
            })
            .collect();
        if let Some(Modal::Search { hits, selected, .. }) = &mut self.modal {
            *hits = found;
            *selected = 0;
        }
    }

    /// Enter num resultado: a relação abre na grade; um schema, ou um nome
    /// que existe em vários, afina a busca (`schema.` / `.tabela`).
    fn open_hit(&mut self, hit: SearchRow) {
        let narrowed = match (hit.kind, hit.many) {
            (CatalogKind::Schema, _) => Some(format!("{}.", hit.name)),
            (_, Some(_)) => Some(format!(".{}", hit.name)),
            _ => None,
        };
        if let Some(query) = narrowed {
            if let Some(Modal::Search { value, .. }) = &mut self.modal {
                *value = query;
            }
            self.search_changed();
            if hit.kind == CatalogKind::Schema {
                // As relações do schema podem não ter chegado ainda
                let state = Arc::clone(&self.state);
                self.spawn(async move {
                    let loaded = async {
                        let entry = state.catalog_entry(hit.server_id, &hit.database).await?;
                        state.catalog.ensure_schemas(&entry, std::slice::from_ref(&hit.name)).await
                    };
                    Msg::CatalogReady(loaded.await.map_err(|error| error.to_string()))
                });
            }
            return;
        }

        self.modal = None;
        let Some(server) = self.servers.iter().find(|server| server.id == Some(hit.server_id)) else {
            return;
        };
        let target = Target {
            server_id: hit.server_id,
            server_name: server.name.clone(),
            db_type: server.db_type,
            database: hit.database,
            schema: hit.schema,
        };
        self.execute(Source::Table {
            target,
            table: hit.name,
        });
        self.focus = Pane::Grid;
    }

    // ── Execução ────────────────────────────────────────────────────────────

    /// Executa o que está no editor, em modo só leitura. Se o guard não
    /// consegue provar que é leitura, pergunta antes de rodar com escrita.
    fn run_editor(&mut self) {
        let text = self.editor.text();
        if text.trim().is_empty() {
            self.set_status(Tone::Warn, "o editor está vazio");
            return;
        }
        let Some(target) = self.target() else {
            self.set_status(Tone::Warn, "escolha um servidor na árvore antes de executar");
            return;
        };

        let language = match capabilities(target.db_type) {
            Ok(capabilities) => capabilities.query_language,
            Err(error) => return self.set_status(Tone::Error, error.to_string()),
        };
        let op = ReadOnlyOp::new(language, text.clone());
        match guard::inspect(&op, target.db_type) {
            Ok(None) => self.execute(Source::Query {
                target,
                text,
                write: false,
            }),
            Ok(Some(rejection)) => self.confirm_write(target, text, &rejection.to_string()),
            Err(error) => self.set_status(Tone::Error, error.to_string()),
        }
    }

    fn confirm_write(&mut self, target: Target, text: String, reason: &str) {
        self.modal = Some(Modal::Confirm {
            lines: vec![
                "Esta consulta não passou no modo só leitura:".into(),
                format!("  {reason}"),
                String::new(),
                format!("Executar com permissão de escrita em {}?", target.label()),
            ],
            action: ConfirmAction::RunWrite { target, text },
        });
    }

    /// Roda `source` do começo e põe o resultado na grade.
    fn execute(&mut self, source: Source) {
        self.ticket += 1;
        self.loading_more = false;
        let ticket = self.ticket;
        let state = Arc::clone(&self.state);
        let sink: Arc<dyn MessageSink> = Arc::new(NoticeSink(self.tx.clone()));
        self.status = None;
        if let Source::Query { target, write: true, .. } = &source {
            log::info!(
                "server {} db {}: escrita confirmada na TUI",
                target.server_id,
                target.database
            );
        }
        self.spawn(async move {
            let result = fetch(state, source.clone(), 0, Some(sink)).await;
            Msg::Result { ticket, source, result }
        });
    }

    fn reload(&mut self) {
        if let Some(source) = self.source.clone() {
            // Recarregar não repete uma escrita
            match source {
                Source::Query { write: true, .. } => self.set_status(Tone::Warn, "uma escrita não é repetida ao recarregar"),
                source => self.execute(source),
            }
        }
    }

    fn load_more(&mut self) {
        let (Some(grid), Some(source)) = (&self.grid, &self.source) else {
            return;
        };
        if self.loading_more || !grid.wants_more() {
            return;
        }
        self.loading_more = true;
        let ticket = self.ticket;
        let offset = grid.loaded_rows() as i64;
        let (state, source) = (Arc::clone(&self.state), source.clone());
        self.spawn(async move {
            Msg::Page {
                ticket,
                result: fetch(state, source, offset, None).await.map_err(|error| error.to_string()),
            }
        });
    }

    fn save(&mut self) {
        let Some(grid) = &self.grid else {
            return;
        };
        if grid.pending_count() == 0 {
            self.set_status(Tone::Info, "nada pendente para gravar");
            return;
        }
        let (Some((info, changes)), Some(source)) = (grid.changes(), &self.source) else {
            return;
        };
        let target = source.target().clone();
        let state = Arc::clone(&self.state);
        log::info!(
            "server {} db {}: gravando alterações pendentes pela TUI",
            target.server_id,
            target.database
        );
        self.spawn(async move {
            let saved = async {
                let adapter = state.adapter(target.server_id, &target.database).await?;
                adapter.save_row_changes(&info, changes).await
            };
            Msg::Saved(saved.await.map(|result| result.affected_rows).map_err(|error| error.to_string()))
        });
    }

    // ── Grade ───────────────────────────────────────────────────────────────

    fn grid_action(&mut self, action: Action) {
        match action {
            Action::QuitNav => return self.request_quit(),
            Action::HelpNav => return self.modal = Some(Modal::Help),
            Action::Refresh => return self.reload(),
            _ => {}
        }
        let page = self.areas.grid.height.saturating_sub(2).max(1) as isize;
        let Some(grid) = self.grid.as_mut() else {
            return;
        };
        let (row, col) = (grid.row, grid.col);
        let mut warn = None;
        match action {
            Action::Up => grid.move_by(-1, 0),
            Action::Down => grid.move_by(1, 0),
            Action::Left => grid.move_by(0, -1),
            Action::Right => grid.move_by(0, 1),
            Action::PageUp => grid.move_by(-page, 0),
            Action::PageDown => grid.move_by(page, 0),
            Action::Top => grid.move_by(isize::MIN / 2, 0),
            Action::Bottom => grid.move_by(isize::MAX / 2, 0),
            Action::Open => {
                if let Some(name) = grid.columns.get(col) {
                    // O nome da coluna e o tipo dela
                    let title = match grid.types.get(col).filter(|type_name| !type_name.is_empty()) {
                        Some(type_name) => format!("{name} · {type_name}"),
                        None => name.clone(),
                    };
                    self.modal = Some(Modal::Cell {
                        title,
                        text: grid.cell(row, col).unwrap_or("NULL").to_string(),
                    });
                }
            }
            Action::Edit | Action::SetNull | Action::DeleteRow | Action::AddRow | Action::Undo => {
                match grid.read_only_reason() {
                    Some(reason) => warn = Some(reason.to_string()),
                    None => match action {
                        Action::Edit if !grid.rows.is_empty() => {
                            self.modal = Some(Modal::Input {
                                title: format!("Editar {}", grid.columns.get(col).cloned().unwrap_or_default()),
                                value: grid.cell(row, col).unwrap_or_default().to_string(),
                                purpose: InputPurpose::EditCell { row, col },
                            })
                        }
                        Action::SetNull => grid.set_cell(row, col, None),
                        Action::DeleteRow => grid.toggle_delete(row),
                        Action::AddRow => grid.add_row(),
                        Action::Undo => grid.undo_row(row),
                        _ => {}
                    },
                }
            }
            _ => {}
        }
        if let Some(reason) = warn {
            self.set_status(Tone::Warn, reason);
        }
        self.load_more();
    }

    // ── Modais ──────────────────────────────────────────────────────────────

    fn modal_key(&mut self, combo: Combo) {
        let Some(modal) = self.modal.clone() else {
            return;
        };
        match modal {
            Modal::Help | Modal::Cell { .. } => self.modal = None,
            Modal::Confirm { action, .. } => {
                self.modal = None;
                let yes = matches!(combo.code, KeyCode::Char('s' | 'S' | 'y' | 'Y'));
                match (yes, action) {
                    (true, ConfirmAction::Quit) => self.quit = true,
                    (true, ConfirmAction::RunWrite { target, text }) => self.execute(Source::Query {
                        target,
                        text,
                        write: true,
                    }),
                    (false, ConfirmAction::RunWrite { .. }) => self.set_status(Tone::Info, "nada foi executado"),
                    (false, ConfirmAction::Quit) => {}
                }
            }
            Modal::Search { hits, selected, .. } => match combo.code {
                KeyCode::Esc => self.modal = None,
                KeyCode::Enter => {
                    if let Some(hit) = hits.get(selected).cloned() {
                        self.open_hit(hit);
                    }
                }
                KeyCode::Up | KeyCode::Down => {
                    let last = hits.len().saturating_sub(1);
                    if let Some(Modal::Search { selected, .. }) = &mut self.modal {
                        *selected = match combo.code {
                            KeyCode::Up => selected.saturating_sub(1),
                            _ => (*selected + 1).min(last),
                        };
                    }
                }
                KeyCode::Backspace => {
                    if let Some(Modal::Search { value, .. }) = &mut self.modal {
                        value.pop();
                    }
                    self.search_changed();
                }
                KeyCode::Char(ch) if combo.types_text() => {
                    if let Some(Modal::Search { value, .. }) = &mut self.modal {
                        value.push(ch);
                    }
                    self.search_changed();
                }
                _ => {}
            },
            Modal::Input { purpose, value, .. } => match combo.code {
                KeyCode::Esc => {
                    self.modal = None;
                    if purpose == InputPurpose::Filter {
                        self.tree.set_filter("");
                    }
                }
                KeyCode::Enter => {
                    self.modal = None;
                    if let (InputPurpose::EditCell { row, col }, Some(grid)) = (purpose, self.grid.as_mut()) {
                        grid.set_cell(row, col, Some(value));
                    }
                }
                KeyCode::Backspace => {
                    if let Some(Modal::Input { value, .. }) = &mut self.modal {
                        value.pop();
                    }
                    self.input_changed();
                }
                KeyCode::Char(ch) if combo.types_text() => {
                    if let Some(Modal::Input { value, .. }) = &mut self.modal {
                        value.push(ch);
                    }
                    self.input_changed();
                }
                _ => {}
            },
        }
    }

    /// O filtro da árvore acompanha o que está sendo digitado.
    fn input_changed(&mut self) {
        if let Some(Modal::Input {
            value,
            purpose: InputPurpose::Filter,
            ..
        }) = &self.modal
        {
            let value = value.clone();
            self.tree.set_filter(&value);
        }
    }

    // ── Mouse (opcional: `[ui] mouse = true`) ───────────────────────────────

    pub fn on_mouse(&mut self, event: MouseEvent) {
        if self.modal.is_some() {
            return;
        }
        let inside = |area: Rect| {
            event.column >= area.x && event.column < area.x + area.width && event.row >= area.y && event.row < area.y + area.height
        };
        let pane = if inside(self.areas.tree) {
            Pane::Tree
        } else if inside(self.areas.editor) {
            Pane::Editor
        } else if inside(self.areas.grid) {
            Pane::Grid
        } else {
            return;
        };

        match event.kind {
            MouseEventKind::Down(MouseButton::Left) => {
                self.focus = pane;
                match pane {
                    Pane::Tree => {
                        let row = self.tree.scroll + (event.row - self.areas.tree.y) as usize;
                        if row < self.tree.len() {
                            self.tree.select(row);
                        }
                    }
                    Pane::Grid => {
                        // A primeira linha da área é o cabeçalho das colunas
                        let offset = (event.row - self.areas.grid.y) as usize;
                        if let (Some(grid), true) = (self.grid.as_mut(), offset >= 1) {
                            let row = grid.scroll_row + offset - 1;
                            if row < grid.rows.len() {
                                grid.row = row;
                            }
                        }
                    }
                    Pane::Editor => {}
                }
            }
            MouseEventKind::ScrollUp | MouseEventKind::ScrollDown => {
                let delta = if event.kind == MouseEventKind::ScrollUp { -3 } else { 3 };
                match pane {
                    Pane::Tree => self.tree.move_by(delta),
                    Pane::Editor => self.editor.move_vertical(delta),
                    Pane::Grid => {
                        if let Some(grid) = self.grid.as_mut() {
                            grid.move_by(delta, 0);
                        }
                        self.load_more();
                    }
                }
            }
            _ => {}
        }
    }

    // ── Respostas das tarefas ───────────────────────────────────────────────

    pub fn on_msg(&mut self, msg: Msg) {
        if !matches!(msg, Msg::Notice(_)) {
            self.busy = self.busy.saturating_sub(1);
        }
        match msg {
            Msg::Children { node, result } => match result {
                Ok(children) => {
                    if children.is_empty() {
                        self.set_status(Tone::Info, "vazio");
                    }
                    self.tree.set_children(node, children);
                }
                Err(error) => {
                    self.tree.set_failed(node, error.clone());
                    self.set_status(Tone::Error, error);
                }
            },
            Msg::Result { ticket, source, result } => {
                if ticket != self.ticket {
                    return;
                }
                match result {
                    Ok(result) => {
                        let mut grid = Grid::from_result(result);
                        if matches!(source, Source::Query { write: true, .. }) {
                            grid.has_more = false;
                        }
                        if self.status.is_none() {
                            let more = if grid.has_more { "+" } else { "" };
                            self.set_status(Tone::Ok, format!("{}{more} linha(s) · {} ms", grid.rows.len(), grid.elapsed_ms));
                        }
                        self.grid = Some(grid);
                        self.source = Some(source);
                    }
                    // O banco recusou o que o guard deixou passar (função que
                    // escreve, por exemplo): a mesma pergunta
                    Err(Error::GuardRejected(reason)) => {
                        if let Source::Query { target, text, .. } = source {
                            self.confirm_write(target, text, &reason);
                        }
                    }
                    Err(error) => self.set_status(Tone::Error, error.to_string()),
                }
            }
            Msg::Page { ticket, result } => {
                if ticket != self.ticket {
                    return;
                }
                self.loading_more = false;
                match (result, self.grid.as_mut()) {
                    (Ok(page), Some(grid)) => grid.append(page),
                    (Err(error), Some(grid)) => {
                        grid.has_more = false;
                        self.set_status(Tone::Error, error);
                    }
                    _ => {}
                }
            }
            Msg::Saved(result) => match result {
                Ok(affected) => {
                    self.reload();
                    self.set_status(Tone::Ok, format!("{affected} linha(s) gravada(s)"));
                }
                Err(error) => self.set_status(Tone::Error, error),
            },
            Msg::CatalogReady(result) => match result {
                Ok(()) => self.search_changed(),
                Err(error) => self.set_status(Tone::Error, error),
            },
            Msg::Notice(message) => self.set_status(Tone::Info, message),
        }
    }
}
