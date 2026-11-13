//! A TUI do `octapus`: árvore de servidores, editor de consulta e grade de
//! resultados, 100% por teclado (mouse opcional), sobre os mesmos dados do app.
//!
//! O laço é **orientado a eventos**: a tela só é redesenhada quando chega uma
//! tecla, uma resposta do banco ou um quadro de animação — e o relógio da
//! animação só roda enquanto algo se mexe (o polvo no painel vazio, o
//! indicador de atividade). Parada, a TUI não gasta CPU. Nenhuma tecla espera
//! o banco: as idas a ele são tarefas em segundo plano (`app.rs`).
//!
//! - `app.rs`: o estado e o que cada tecla faz;
//! - `view.rs`: o desenho de um quadro;
//! - `tree.rs`, `editor.rs`, `grid.rs`: os três painéis;
//! - `config.rs`, `keys.rs`: tema e atalhos do `config.toml`;
//! - `mascot.rs`: o polvo.

mod app;
mod config;
mod editor;
mod grid;
mod keys;
mod mascot;
#[cfg(test)]
mod tests;
mod tree;
mod view;

use std::io::{stdout, IsTerminal};
use std::sync::Arc;
use std::time::{Duration, Instant};

use octapus_core::paths;
use ratatui::crossterm::event::{
    self, DisableBracketedPaste, DisableMouseCapture, EnableBracketedPaste, EnableMouseCapture, Event,
};
use ratatui::crossterm::execute;
use ratatui::crossterm::terminal::{BeginSynchronizedUpdate, EndSynchronizedUpdate};
use tokio::sync::mpsc::unbounded_channel;

use crate::cli::TuiArgs;
use crate::context::App;
use crate::error::{CliError, CliResult};

use app::{Msg, Tone, Tui};
use config::{ColorDepth, ConfigFile, Theme};
use keys::KeyMap;

/// Intervalo dos quadros de animação (só enquanto algo se mexe).
const ANIMATION_FRAME: Duration = Duration::from_millis(90);

/// Tempos de desenho, para o log (o orçamento de desempenho da TUI).
#[derive(Default)]
struct FrameStats {
    micros: Vec<u32>,
}

impl FrameStats {
    fn record(&mut self, elapsed: Duration) {
        // Amostra limitada: uma sessão longa não cresce sem fim
        if self.micros.len() < 100_000 {
            self.micros.push(elapsed.as_micros().min(u32::MAX as u128) as u32);
        }
    }

    fn log(mut self, first_frame: Duration) {
        if self.micros.is_empty() {
            return;
        }
        self.micros.sort_unstable();
        let at = |quantile: f32| self.micros[((self.micros.len() - 1) as f32 * quantile) as usize];
        log::info!(
            "tui: primeiro quadro em {} ms; {} quadros, desenho p50 {} µs, p99 {} µs",
            first_frame.as_millis(),
            self.micros.len(),
            at(0.5),
            at(0.99)
        );
    }
}

pub async fn run(app: App, args: TuiArgs) -> CliResult<()> {
    let started = Instant::now();
    if !std::io::stdin().is_terminal() || !stdout().is_terminal() {
        return Err(CliError::Usage(
            "a TUI precisa de um terminal; fora dele use os subcomandos (octapus --help)".into(),
        ));
    }

    // Configuração: um arquivo com erro não impede de abrir
    let mut warnings = Vec::new();
    let file = match paths::config_dir().map_err(|error| error.to_string()).and_then(|dir| ConfigFile::load(&dir)) {
        Ok(file) => file,
        Err(error) => {
            warnings.push(error);
            ConfigFile::default()
        }
    };
    let theme = Theme::new(&file.ui.theme, &file.theme, ColorDepth::detect(), &mut warnings);
    let keys = KeyMap::new(&file.keys, &mut warnings);
    let mouse = file.ui.mouse;

    let (tx, mut rx) = unbounded_channel::<Msg>();
    let state = Arc::new(app.state);
    let mut tui = Tui::new(state, theme, keys, file.ui, tx, tokio::runtime::Handle::current());
    if let Some(warning) = warnings.first() {
        let more = match warnings.len() {
            1 => String::new(),
            count => format!(" (+{} aviso(s))", count - 1),
        };
        tui.set_status(Tone::Warn, format!("config.toml: {warning}{more}"));
    }
    if let Some(reference) = &args.server {
        tui.select_server(reference);
    }

    // A leitura do teclado bloqueia: fica numa thread própria
    let (event_tx, mut events) = unbounded_channel::<Event>();
    std::thread::spawn(move || {
        while let Ok(event) = event::read() {
            if event_tx.send(event).is_err() {
                break;
            }
        }
    });

    let mut terminal = ratatui::try_init()?;
    let _ = execute!(stdout(), EnableBracketedPaste);
    if mouse {
        let _ = execute!(stdout(), EnableMouseCapture);
    }

    let mut ticker = tokio::time::interval(ANIMATION_FRAME);
    ticker.set_missed_tick_behavior(tokio::time::MissedTickBehavior::Skip);
    let mut stats = FrameStats::default();
    let mut first_frame = None;

    let outcome: CliResult<()> = loop {
        // Cada volta do laço é um evento (tecla, resposta, quadro de animação):
        // um desenho por evento, nenhum sem evento
        {
            let drawing = Instant::now();
            // Atualização sincronizada: o terminal mostra o quadro inteiro de
            // uma vez (sem piscar em SSH e tmux)
            let _ = execute!(stdout(), BeginSynchronizedUpdate);
            let drawn = terminal.draw(|frame| view::draw(&mut tui, frame)).map(|_| ());
            let _ = execute!(stdout(), EndSynchronizedUpdate);
            if let Err(error) = drawn {
                break Err(error.into());
            }
            stats.record(drawing.elapsed());
            first_frame.get_or_insert_with(|| started.elapsed());
        }

        tokio::select! {
            Some(event) = events.recv() => {
                handle_event(&mut tui, event);
                // Teclas em rajada (repetição, colar) viram um quadro só
                while let Ok(event) = events.try_recv() {
                    handle_event(&mut tui, event);
                }
            }
            Some(msg) = rx.recv() => {
                tui.on_msg(msg);
                while let Ok(msg) = rx.try_recv() {
                    tui.on_msg(msg);
                }
            }
            _ = ticker.tick(), if tui.animating() => {
                tui.tick();
            }
        }
        if tui.quit {
            break Ok(());
        }
    };

    if mouse {
        let _ = execute!(stdout(), DisableMouseCapture);
    }
    let _ = execute!(stdout(), DisableBracketedPaste);
    ratatui::restore();
    stats.log(first_frame.unwrap_or_default());
    outcome
}

fn handle_event(tui: &mut Tui, event: Event) {
    match event {
        Event::Key(key) => tui.on_key(key),
        Event::Paste(text) => tui.on_paste(&text),
        Event::Mouse(mouse) => tui.on_mouse(mouse),
        // Redimensionar e foco só pedem um quadro novo
        _ => {}
    }
}
