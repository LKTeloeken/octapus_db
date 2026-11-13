//! A TUI inteira sem terminal: um `AppState` de verdade sobre uma pasta
//! descartável e um SQLite temporário, dirigida por teclas, com o que as
//! tarefas em segundo plano devolvem e o desenho num terminal de teste.

use std::collections::BTreeMap;
use std::path::PathBuf;
use std::sync::Arc;
use std::time::Duration;

use octapus_core::services::connection_import::server_input_from_url;
use octapus_core::state::AppState;
use octapus_core::storage::repositories::servers;
use ratatui::backend::TestBackend;
use ratatui::crossterm::event::{KeyCode, KeyEvent, KeyModifiers};
use ratatui::style::Color;
use ratatui::Terminal;
use tokio::sync::mpsc::{unbounded_channel, UnboundedReceiver};

use super::app::{Modal, Msg, Pane, Tui};
use super::config::{ColorDepth, Theme, UiConfig};
use super::keys::KeyMap;
use super::view;

struct Harness {
    tui: Tui,
    rx: UnboundedReceiver<Msg>,
    terminal: Terminal<TestBackend>,
    dir: PathBuf,
}

impl Drop for Harness {
    fn drop(&mut self) {
        let _ = std::fs::remove_dir_all(&self.dir);
    }
}

impl Harness {
    /// Uma TUI sobre um SQLite com `customers` (3 linhas) e `big` (1.200),
    /// já cadastrado como o servidor `loja`.
    fn new(label: &str) -> Self {
        let dir = std::env::temp_dir().join(format!("octapus-tui-{label}-{}", std::process::id()));
        let _ = std::fs::remove_dir_all(&dir);
        std::fs::create_dir_all(&dir).unwrap();
        let database = dir.join("loja.db");
        rusqlite::Connection::open(&database)
            .unwrap()
            .execute_batch(
                "CREATE TABLE customers (id INTEGER PRIMARY KEY, name TEXT NOT NULL, city TEXT);
                 INSERT INTO customers (name, city) VALUES ('Ana', 'Porto Alegre'), ('Bia', NULL), ('Caio', 'Curitiba');
                 CREATE TABLE big (id INTEGER PRIMARY KEY, label TEXT);
                 WITH RECURSIVE n(i) AS (SELECT 1 UNION ALL SELECT i + 1 FROM n WHERE i < 1200)
                 INSERT INTO big (label) SELECT 'linha ' || i FROM n;",
            )
            .unwrap();

        let state = AppState::open(&dir.join("data")).unwrap();
        state.catalog.init(dir.join("data/catalog"), Arc::new(|_| {}));
        let mut input = server_input_from_url(database.to_str().unwrap()).unwrap();
        input.name = "loja".into();
        servers::create(&state.storage, input).unwrap();

        let mut warnings = Vec::new();
        let none = BTreeMap::new();
        let (tx, rx) = unbounded_channel();
        let tui = Tui::new(
            Arc::new(state),
            Theme::new("dark", &none, ColorDepth::TrueColor, &mut warnings),
            KeyMap::new(&BTreeMap::new(), &mut warnings),
            UiConfig {
                mascot: false,
                ..UiConfig::default()
            },
            tx,
            tokio::runtime::Handle::current(),
        );
        assert!(warnings.is_empty(), "{warnings:?}");
        Self {
            tui,
            rx,
            terminal: Terminal::new(TestBackend::new(100, 30)).unwrap(),
            dir,
        }
    }

    fn key(&mut self, code: KeyCode) {
        self.tui.on_key(KeyEvent::new(code, KeyModifiers::NONE));
    }

    fn ctrl(&mut self, ch: char) {
        self.tui.on_key(KeyEvent::new(KeyCode::Char(ch), KeyModifiers::CONTROL));
    }

    fn keys(&mut self, text: &str) {
        for ch in text.chars() {
            self.key(KeyCode::Char(ch));
        }
    }

    /// Espera as tarefas em andamento terminarem e aplica o que devolveram.
    async fn settle(&mut self) {
        while self.tui.busy > 0 {
            let msg = tokio::time::timeout(Duration::from_secs(10), self.rx.recv())
                .await
                .expect("a tarefa não respondeu")
                .expect("canal fechado");
            self.tui.on_msg(msg);
        }
    }

    /// Desenha um quadro e devolve as linhas da tela.
    fn screen(&mut self) -> Vec<String> {
        let tui = &mut self.tui;
        self.terminal.draw(|frame| view::draw(tui, frame)).unwrap();
        let buffer = self.terminal.backend().buffer();
        let area = *buffer.area();
        (0..area.height)
            .map(|y| (0..area.width).map(|x| buffer[(x, y)].symbol()).collect::<String>())
            .collect()
    }

    fn screen_text(&mut self) -> String {
        self.screen().join("\n")
    }

    /// A cor de fundo da primeira célula da tela que mostra `text`.
    fn background_of(&mut self, text: &str) -> Color {
        let lines = self.screen();
        let (y, line) = lines
            .iter()
            .enumerate()
            .find(|(_, line)| line.contains(text))
            .unwrap_or_else(|| panic!("\"{text}\" não está na tela:\n{}", lines.join("\n")));
        let x = line[..line.find(text).unwrap()].chars().count();
        self.terminal.backend().buffer()[(x as u16, y as u16)].bg
    }

    /// Abre servidor → database → tabela `table` na grade.
    async fn open_table(&mut self, table: &str) {
        self.key(KeyCode::Enter);
        self.settle().await;
        self.key(KeyCode::Down);
        self.key(KeyCode::Enter);
        self.settle().await;
        while self.tui.tree.selected_node().is_none_or(|(_, node)| node.label != table) {
            let before = self.tui.tree.selected;
            self.key(KeyCode::Down);
            assert_ne!(before, self.tui.tree.selected, "tabela {table} não está na árvore");
        }
        self.key(KeyCode::Enter);
        self.settle().await;
    }
}

#[tokio::test]
async fn opens_a_table_from_the_tree() {
    let mut harness = Harness::new("open");
    let screen = harness.screen_text();
    assert!(screen.contains("SERVIDORES"), "{screen}");
    assert!(screen.contains("▸ sl loja"), "{screen}");

    harness.open_table("customers").await;
    assert_eq!(harness.tui.focus, Pane::Grid);
    let screen = harness.screen_text();
    assert!(screen.contains("▾ sl loja"), "{screen}");
    assert!(screen.contains("RESULTADOS · 3 linha(s)"), "{screen}");
    assert!(screen.contains("Porto Alegre"), "{screen}");
    assert!(screen.contains("NULL"), "{screen}");
    // O contexto do editor acompanha o nó selecionado
    assert!(screen.contains("CONSULTA · loja › main"), "{screen}");
}

#[tokio::test]
async fn editor_runs_read_only_and_asks_before_writing() {
    let mut harness = Harness::new("query");
    harness.key(KeyCode::F(3));
    harness.keys("select name from customers where city is null");
    harness.key(KeyCode::F(5));
    harness.settle().await;
    let screen = harness.screen_text();
    assert!(screen.contains("RESULTADOS · 1 linha(s)"), "{screen}");
    assert!(screen.contains("Bia"), "{screen}");

    // Uma escrita não roda sozinha: a TUI pergunta
    for _ in 0..80 {
        harness.key(KeyCode::Backspace);
    }
    harness.keys("delete from customers where id = 3");
    harness.key(KeyCode::F(5));
    assert!(matches!(harness.tui.modal, Some(Modal::Confirm { .. })));
    assert_eq!(harness.tui.busy, 0, "nada roda antes da resposta");
    assert!(harness.screen_text().contains("Executar com permissão de escrita em loja › main?"));

    // Qualquer tecla que não seja "s" desiste
    harness.key(KeyCode::Enter);
    harness.settle().await;
    assert!(harness.tui.modal.is_none());
    let count = |dir: &PathBuf| -> i64 {
        rusqlite::Connection::open(dir.join("loja.db"))
            .unwrap()
            .query_row("SELECT count(*) FROM customers", [], |row| row.get(0))
            .unwrap()
    };
    assert_eq!(count(&harness.dir), 3);

    harness.key(KeyCode::F(5));
    harness.key(KeyCode::Char('s'));
    harness.settle().await;
    assert_eq!(count(&harness.dir), 2);
}

#[tokio::test]
async fn pending_edits_are_colored_and_saved_in_one_go() {
    let mut harness = Harness::new("edit");
    harness.open_table("customers").await;
    // A tabela abre com as linhas mais novas primeiro: 3 Caio, 2 Bia, 1 Ana

    // Edita o nome da primeira linha
    harness.key(KeyCode::Right);
    harness.key(KeyCode::Char('e'));
    assert!(matches!(harness.tui.modal, Some(Modal::Input { .. })));
    for _ in 0..4 {
        harness.key(KeyCode::Backspace);
    }
    harness.keys("Zeca Maria");
    harness.key(KeyCode::Enter);
    // Marca a segunda para excluir e acrescenta uma nova
    harness.key(KeyCode::Down);
    harness.key(KeyCode::Char('d'));
    harness.key(KeyCode::Char('a'));
    harness.key(KeyCode::Char('e'));
    harness.keys("Davi");
    harness.key(KeyCode::Enter);
    // O cursor sai da linha nova, para o fundo dela ser só o da pendência
    harness.key(KeyCode::Up);

    let screen = harness.screen_text();
    assert!(screen.contains("3 pendente(s)"), "{screen}");
    let (plain, edited, removed, added) = (
        harness.background_of("Curitiba"),
        harness.background_of("Zeca Maria"),
        harness.background_of("Bia"),
        harness.background_of("Davi"),
    );
    // Três fundos diferentes entre si e do fundo normal: amarelo, vermelho, verde
    let all = [plain, edited, removed, added];
    for (index, color) in all.iter().enumerate() {
        assert!(all.iter().skip(index + 1).all(|other| other != color), "{all:?}");
    }

    // Nada foi ao banco ainda
    let names = |dir: &PathBuf| -> Vec<String> {
        let connection = rusqlite::Connection::open(dir.join("loja.db")).unwrap();
        let mut statement = connection.prepare("SELECT name FROM customers ORDER BY id").unwrap();
        let rows = statement.query_map([], |row| row.get(0)).unwrap();
        rows.map(Result::unwrap).collect()
    };
    assert_eq!(names(&harness.dir), ["Ana", "Bia", "Caio"]);

    harness.ctrl('s');
    harness.settle().await;
    assert_eq!(names(&harness.dir), ["Ana", "Zeca Maria", "Davi"]);
    let screen = harness.screen_text();
    assert!(!screen.contains("pendente"), "{screen}");
    assert!(screen.contains("gravada(s)"), "{screen}");
}

#[tokio::test]
async fn large_results_arrive_page_by_page() {
    let mut harness = Harness::new("pages");
    harness.open_table("big").await;
    assert_eq!(harness.tui.grid.as_ref().unwrap().rows.len(), 500);
    assert!(harness.screen_text().contains("RESULTADOS · 500 linha(s)+"));

    // Ir para o fim pede a página seguinte, e de novo até acabar
    for _ in 0..4 {
        harness.key(KeyCode::Char('G'));
        harness.settle().await;
    }
    let grid = harness.tui.grid.as_ref().unwrap();
    assert_eq!(grid.rows.len(), 1200);
    assert!(!grid.has_more);
    let screen = harness.screen_text();
    // As mais novas vêm primeiro: o fim da grade é a linha 1
    assert!(screen.contains("linha 14"), "{screen}");
    // Só o que cabe na tela é desenhado
    assert!(!screen.contains("linha 900"), "{screen}");
}

#[tokio::test]
async fn filter_help_and_quit() {
    let mut harness = Harness::new("misc");
    harness.key(KeyCode::Enter);
    harness.settle().await;
    harness.key(KeyCode::Down);
    harness.key(KeyCode::Enter);
    harness.settle().await;

    harness.key(KeyCode::Char('/'));
    harness.keys("cust");
    let screen = harness.screen_text();
    assert!(screen.contains("customers") && !screen.contains("▤ big"), "{screen}");
    harness.key(KeyCode::Esc);
    assert!(harness.screen_text().contains("▤ big"));

    harness.key(KeyCode::F(1));
    assert!(harness.screen_text().contains("executar a consulta"));
    harness.key(KeyCode::Esc);

    // No editor, "q" é texto; na árvore, sai
    harness.key(KeyCode::F(3));
    harness.key(KeyCode::Char('q'));
    assert!(!harness.tui.quit);
    harness.key(KeyCode::F(2));
    harness.key(KeyCode::Char('q'));
    assert!(harness.tui.quit);
}

/// O orçamento de desenho: um quadro com a grade cheia e a árvore com 5.000
/// nós tem que caber folgado num quadro de 60 Hz, mesmo em build de teste.
#[tokio::test]
async fn frame_time_does_not_depend_on_how_much_is_loaded() {
    let mut harness = Harness::new("perf");
    harness.open_table("big").await;
    harness.screen();

    let started = std::time::Instant::now();
    const FRAMES: u32 = 200;
    for _ in 0..FRAMES {
        harness.key(KeyCode::Down);
        harness.screen();
    }
    let per_frame = started.elapsed() / FRAMES;
    assert!(per_frame < Duration::from_millis(8), "quadro em {per_frame:?}");
}

#[tokio::test]
async fn search_opens_a_table_from_the_catalog() {
    let mut harness = Harness::new("search");
    // Com o cursor só no servidor: a busca abre o catálogo do database padrão
    harness.ctrl('p');
    harness.settle().await;
    harness.keys("cust");
    let Some(Modal::Search { hits, .. }) = &harness.tui.modal else {
        panic!("a busca não abriu");
    };
    assert_eq!(hits.first().map(|hit| hit.name.as_str()), Some("customers"), "{hits:?}");
    let screen = harness.screen_text();
    assert!(screen.contains("Buscar no catálogo"), "{screen}");
    assert!(screen.contains("▤ customers"), "{screen}");

    harness.key(KeyCode::Enter);
    harness.settle().await;
    assert!(harness.tui.modal.is_none());
    assert_eq!(harness.tui.focus, Pane::Grid);
    let screen = harness.screen_text();
    assert!(screen.contains("RESULTADOS · 3 linha(s)"), "{screen}");
    assert!(screen.contains("Porto Alegre"), "{screen}");
}

#[tokio::test]
async fn refresh_rereads_the_structure_from_the_database() {
    let mut harness = Harness::new("refresh");
    harness.key(KeyCode::Enter);
    harness.settle().await;
    harness.key(KeyCode::Down);
    harness.key(KeyCode::Enter);
    harness.settle().await;
    assert!(!harness.screen_text().contains("invoices"));

    rusqlite::Connection::open(harness.dir.join("loja.db"))
        .unwrap()
        .execute_batch("CREATE TABLE invoices (id INTEGER PRIMARY KEY);")
        .unwrap();
    // Recolher e abrir de novo mostra o que o catálogo já tem
    harness.key(KeyCode::Enter);
    harness.key(KeyCode::Enter);
    harness.settle().await;
    assert!(!harness.screen_text().contains("invoices"));

    harness.keys("r");
    harness.settle().await;
    let screen = harness.screen_text();
    assert!(screen.contains("▤ invoices"), "{screen}");
    assert!(screen.contains("▤ customers"), "{screen}");
}

#[tokio::test]
async fn a_newer_version_shows_in_the_status_bar() {
    let mut harness = Harness::new("update");
    harness.tui.on_msg(Msg::UpdateAvailable("9.9.9".into()));
    assert_eq!(harness.tui.busy, 0);
    let screen = harness.screen_text();
    assert!(screen.contains("octapus 9.9.9 disponível: rode octapus update"), "{screen}");
}

#[tokio::test]
async fn grid_colors_and_aligns_values_by_column_type() {
    let mut harness = Harness::new("types");
    harness.open_table("customers").await;
    let lines = harness.screen();
    // O cabeçalho tem o nome e, embaixo, o tipo
    let header = lines.iter().position(|line| line.contains("│ name")).expect("cabeçalho");
    assert!(lines[header + 1].contains("integer") && lines[header + 1].contains("text"), "{}", lines[header + 1]);

    // Número: à direita (colado no divisor) e na cor do tipo; texto, na cor normal
    let (y, line) = lines.iter().enumerate().find(|(_, line)| line.contains("1 │ Ana")).expect("id à direita");
    let x = line[..line.find("1 │ Ana").unwrap()].chars().count() as u16;
    let buffer = harness.terminal.backend().buffer();
    assert_eq!(buffer[(x, y as u16)].fg, Color::Rgb(0x82, 0xc7, 0xf0));
    assert_eq!(buffer[(x + 4, y as u16)].fg, Color::Rgb(0xed, 0xee, 0xf1));

    // Pendência: cor e símbolo na coluna do número da linha
    harness.key(KeyCode::Char('d'));
    let lines = harness.screen();
    assert!(lines.iter().any(|line| line.contains("−1 │")), "{}", lines.join("\n"));
}

#[tokio::test]
async fn panes_get_boxes_only_when_the_terminal_has_room() {
    let mut harness = Harness::new("boxes");
    let screen = harness.screen_text();
    assert!(screen.contains("╭ SERVIDORES ─"), "{screen}");
    assert!(screen.contains("╭ RESULTADOS ─"), "{screen}");
    // O painel com foco leva o acento na borda; os outros, a linha apagada
    let buffer = harness.terminal.backend().buffer();
    assert_eq!(buffer[(0, 0)].fg, Color::Rgb(0xa8, 0xaa, 0xff));
    assert_eq!(buffer[(99, 0)].fg, Color::Rgb(0x2b, 0x2d, 0x32));

    // Estreito: volta ao layout compacto, sem gastar colunas com borda
    harness.terminal = Terminal::new(TestBackend::new(80, 24)).unwrap();
    harness.open_table("customers").await;
    let screen = harness.screen_text();
    assert!(!screen.contains('╭'), "{screen}");
    assert!(screen.contains(" SERVIDORES"), "{screen}");
    assert!(screen.contains("Porto Alegre"), "{screen}");
}

#[tokio::test]
async fn autocomplete_follows_the_query_context() {
    let mut harness = Harness::new("complete");
    harness.key(KeyCode::F(3));
    let labels = |harness: &Harness| -> Vec<String> {
        harness.tui.editor.completion.as_ref().map_or(Vec::new(), |completion| completion.items.iter().map(|item| item.label.clone()).collect())
    };

    // Uma letra não abre a lista; duas, sim
    harness.keys("s");
    assert!(harness.tui.editor.completion.is_none());
    harness.keys("e");
    assert!(labels(&harness).contains(&"select".to_string()));
    harness.key(KeyCode::Enter);
    assert_eq!(harness.tui.editor.text(), "select");

    // Depois de FROM: as tabelas do catálogo (aberto em segundo plano), sem palavras-chave
    harness.keys(" * from cu");
    harness.settle().await;
    assert_eq!(labels(&harness), ["customers"]);
    let screen = harness.screen_text();
    assert!(screen.contains("▤ customers"), "{screen}");
    harness.key(KeyCode::Tab);
    assert_eq!(harness.tui.editor.text(), "select * from customers");

    // Depois de WHERE: as colunas da tabela citada, com o tipo ao lado
    harness.keys(" where ci");
    harness.settle().await;
    assert_eq!(labels(&harness).first().map(String::as_str), Some("city"));
    assert!(harness.screen_text().contains("▪ city"));
    harness.key(KeyCode::Enter);
    assert_eq!(harness.tui.editor.text(), "select * from customers where city");

    // `tabela.` lista as colunas dela, já lidas: nada vai ao banco de novo
    harness.keys(" = customers.");
    assert_eq!(harness.tui.busy, 0);
    assert_eq!(labels(&harness), ["id", "name", "city"]);
    harness.key(KeyCode::Esc);
    assert!(harness.tui.editor.completion.is_none());

    // Dentro de um texto nada é sugerido, e o Enter sem lista quebra a linha
    harness.keys("'na");
    assert!(harness.tui.editor.completion.is_none());
    harness.keys("'");
    harness.key(KeyCode::Enter);
    assert_eq!(harness.tui.editor.line_count(), 2);

    // Desligado, só o atalho abre
    harness.tui.autocomplete = false;
    harness.keys("sel");
    assert!(harness.tui.editor.completion.is_none());
    harness.ctrl('n');
    assert!(harness.tui.editor.text().ends_with("select"), "um candidato só entra direto");
}

#[tokio::test]
async fn autocomplete_keeps_up_with_typing_on_a_large_catalog() {
    let mut harness = Harness::new("complete-perf");
    let tables: String = (0..3000).map(|index| format!("CREATE TABLE tenant_{index:04} (id INTEGER PRIMARY KEY, total NUMERIC);")).collect();
    rusqlite::Connection::open(harness.dir.join("loja.db")).unwrap().execute_batch(&tables).unwrap();

    harness.key(KeyCode::F(3));
    harness.keys("select * from te");
    harness.settle().await;
    assert!(harness.tui.editor.completion.is_some(), "o catálogo abriu e a lista apareceu");
    harness.screen();

    // Cada tecla: refazer a lista (3.000 tabelas no catálogo) e desenhar
    let typed = "nant_29";
    let started = std::time::Instant::now();
    for ch in typed.chars() {
        harness.key(KeyCode::Char(ch));
        harness.screen();
    }
    let per_key = started.elapsed() / typed.len() as u32;
    let completion = harness.tui.editor.completion.as_ref().expect("lista aberta");
    assert_eq!(completion.items.len(), 50, "o teto da lista");
    assert_eq!(completion.items[0].label, "tenant_2900");
    assert!(per_key < Duration::from_millis(16), "tecla até a tela em {per_key:?}");
    println!("tecla até a tela: {per_key:?}");
}
