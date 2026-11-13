//! O desenho da TUI. Cada quadro escreve direto no buffer só o que cabe na
//! tela (as linhas visíveis da árvore, do editor e da grade): o custo de um
//! quadro não depende do tamanho do resultado nem da árvore.
//!
//! ```text
//! ┌ SERVIDORES ─┬ CONSULTA · loja › main ──────────────┐
//! │ ▾ pg loja   │  1 select * from customers           │
//! │   ▸ ▪ main  ├ RESULTADOS · 3 linhas ───────────────┤
//! │             │ id  name   city                      │
//! └─────────────┴──────────────────────────────────────┘
//!  status                                      atalhos
//! ```

use ratatui::buffer::Buffer;
use ratatui::layout::{Position, Rect};
use ratatui::style::{Modifier, Style};
use ratatui::widgets::{Block, BorderType, Clear, Paragraph, Wrap};
use ratatui::Frame;
use unicode_width::{UnicodeWidthChar, UnicodeWidthStr};

use super::app::{Areas, Modal, Pane, Tone, Tui};
use super::complete::ItemKind;
use super::config::{CellKind, Pending};
use super::grid::{shown, COLUMN_CHROME};
use super::editor::Token;
use octapus_core::models::NodeKind as CatalogKind;
use super::keys::Action;
use super::tree::{Load, NodeKind};

/// A partir deste tamanho de terminal os painéis ganham caixa.
const BOXED_MIN_WIDTH: u16 = 100;
const BOXED_MIN_HEIGHT: u16 = 20;

/// Linhas do cabeçalho da grade: o nome da coluna e o tipo.
pub const GRID_HEADER_LINES: u16 = 2;

const SPINNER: [&str; 8] = ["⠋", "⠙", "⠹", "⠸", "⠼", "⠴", "⠦", "⠇"];

/// Escreve `text` em até `width` colunas a partir de `x`; devolve onde parou.
fn put(buf: &mut Buffer, x: u16, y: u16, width: u16, text: &str, style: Style) -> u16 {
    if width == 0 {
        return x;
    }
    buf.set_stringn(x, y, text, width as usize, style).0
}

/// O texto cortado em `width` colunas, com `…` se não coube.
fn fit(text: &str, width: usize) -> String {
    if text.width() <= width {
        return text.to_string();
    }
    let mut out = String::new();
    let mut used = 0;
    for ch in text.chars() {
        let ch_width = ch.width().unwrap_or(0);
        if used + ch_width + 1 > width {
            break;
        }
        used += ch_width;
        out.push(ch);
    }
    out.push('…');
    out
}

fn fill(buf: &mut Buffer, area: Rect, style: Style) {
    buf.set_style(area, style);
}

/// A moldura de um painel. Com espaço, uma caixa de cantos arredondados com
/// o título na borda (a do painel com foco no acento); num terminal
/// estreito, só a faixa do título — a borda custaria duas colunas por painel.
fn pane_header(buf: &mut Buffer, area: Rect, title: &str, focused: bool, tui: &Tui) {
    if !tui.areas.boxed {
        let style = tui.theme.header(focused);
        fill(buf, Rect::new(area.x, area.y, area.width, 1), style);
        put(buf, area.x + 1, area.y, area.width.saturating_sub(2), title, style);
        return;
    }
    if area.width < 4 || area.height < 2 {
        return;
    }
    let border = tui.theme.border(focused);
    let (left, right, bottom) = (area.x, area.x + area.width - 1, area.y + area.height - 1);
    let line = "─".repeat(area.width as usize - 2);
    put(buf, left, area.y, area.width, &format!("╭{line}╮"), border);
    put(buf, left, bottom, area.width, &format!("╰{line}╯"), border);
    for y in area.y + 1..bottom {
        put(buf, left, y, 1, "│", border);
        put(buf, right, y, 1, "│", border);
    }
    let room = area.width as usize - 4;
    put(buf, left + 1, area.y, area.width - 2, &format!(" {} ", fit(title, room.saturating_sub(2))), tui.theme.title(focused));
}

pub fn draw(tui: &mut Tui, frame: &mut Frame) {
    let area = frame.area();
    if area.width < 30 || area.height < 8 {
        frame.render_widget(Paragraph::new("terminal pequeno demais"), area);
        return;
    }
    fill(frame.buffer_mut(), area, tui.theme.text());

    let boxed = area.width >= BOXED_MIN_WIDTH && area.height >= BOXED_MIN_HEIGHT;
    let body_height = area.height - 1;
    let tree_width = (area.width * 3 / 10).clamp(22, 44).min(area.width / 2);
    // Na caixa, duas linhas do editor são da borda
    let editor_height = (body_height * 35 / 100).clamp(4, 16) + if boxed { 1 } else { 0 };
    // Compacto: uma coluna de divisor entre a árvore e o resto
    let gap = if boxed { 0 } else { 1 };
    let right_x = area.x + tree_width + gap;
    let right_width = area.width - tree_width - gap;

    let tree = Rect::new(area.x, area.y, tree_width, body_height);
    let editor = Rect::new(right_x, area.y, right_width, editor_height);
    let grid = Rect::new(right_x, area.y + editor_height, right_width, body_height - editor_height);

    // As áreas de conteúdo: dentro da caixa, ou abaixo da faixa do título
    let content = |rect: Rect| match boxed {
        true => Rect::new(rect.x + 1, rect.y + 1, rect.width.saturating_sub(2), rect.height.saturating_sub(2)),
        false => Rect::new(rect.x, rect.y + 1, rect.width, rect.height.saturating_sub(1)),
    };
    tui.areas = Areas {
        tree: content(tree),
        editor: content(editor),
        grid: content(grid),
        boxed,
    };

    let buf = frame.buffer_mut();
    if !boxed {
        for y in area.y..area.y + body_height {
            put(buf, area.x + tree_width, y, 1, "│", tui.theme.divider());
        }
    }

    draw_tree(tui, buf, tree);
    let cursor = draw_editor(tui, buf, editor);
    draw_grid(tui, buf, grid);
    draw_status(tui, buf, Rect::new(area.x, area.y + body_height, area.width, 1));

    match tui.modal.clone() {
        Some(modal) => draw_modal(tui, frame, &modal),
        None => {
            if let (Pane::Editor, Some(position)) = (tui.focus, cursor) {
                draw_completion(tui, frame.buffer_mut(), position, editor);
                frame.set_cursor_position(position);
            }
        }
    }
}

fn draw_tree(tui: &mut Tui, buf: &mut Buffer, area: Rect) {
    let focused = tui.focus == Pane::Tree;
    let title = match tui.tree.filter() {
        "" => "SERVIDORES".to_string(),
        filter => format!("SERVIDORES · /{filter}"),
    };
    pane_header(buf, area, &title, focused, tui);

    let rows = tui.areas.tree;
    tui.tree.scroll_into_view(rows.height as usize);
    if tui.tree.is_empty() {
        put(buf, rows.x + 1, rows.y + 1, rows.width.saturating_sub(2), "nenhum servidor salvo", tui.theme.muted());
        put(
            buf,
            rows.x + 1,
            rows.y + 2,
            rows.width.saturating_sub(2),
            "octapus server add <url>",
            tui.theme.subtle(),
        );
        return;
    }

    for line in 0..rows.height as usize {
        let row = tui.tree.scroll + line;
        let Some((_, node)) = tui.tree.at(row) else {
            break;
        };
        let y = rows.y + line as u16;
        let selected = row == tui.tree.selected;
        let base = if selected { tui.theme.selection(focused) } else { tui.theme.text() };
        if selected {
            fill(buf, Rect::new(rows.x, y, rows.width, 1), base);
        }

        let marker = match (node.kind.is_leaf(), node.load, node.expanded) {
            (true, _, _) => "  ",
            (_, Load::Loading, _) => "… ",
            (_, Load::Failed, _) => "! ",
            (_, _, true) => "▾ ",
            (_, _, false) => "▸ ",
        };
        let indent = (node.depth as usize * 2).min(rows.width as usize / 2);
        let mut x = rows.x + 1 + indent as u16;
        let end = rows.x + rows.width;
        // Guias: uma linha fina por nível, para o olho seguir a hierarquia
        for level in 0..indent / 2 {
            put(buf, rows.x + 1 + level as u16 * 2, y, 1, "│", tui.theme.divider().bg(base.bg.unwrap_or_default()));
        }
        let marker_style = match node.load {
            Load::Failed => tui.theme.tone("danger").bg(base.bg.unwrap_or_default()),
            _ => base.patch(tui.theme.subtle().bg(base.bg.unwrap_or_default())),
        };
        x = put(buf, x, y, end.saturating_sub(x), marker, marker_style);
        let glyph_tone = match &node.kind {
            NodeKind::Server { db_type, .. } => tui.theme.database(*db_type),
            NodeKind::Database { .. } => tui.theme.tone("accent_text"),
            NodeKind::Schema { .. } => tui.theme.muted(),
            NodeKind::Table { .. } => tui.theme.subtle(),
        };
        let glyph_style = base.patch(glyph_tone.bg(base.bg.unwrap_or_default()));
        x = put(buf, x, y, end.saturating_sub(x), node.kind.glyph(), glyph_style);
        x = put(buf, x, y, end.saturating_sub(x), " ", base);
        let room = end.saturating_sub(x) as usize;
        put(buf, x, y, room as u16, &fit(&node.label, room), base);
    }
}

/// Desenha o editor e devolve onde fica o cursor do terminal.
fn draw_editor(tui: &mut Tui, buf: &mut Buffer, area: Rect) -> Option<Position> {
    let focused = tui.focus == Pane::Editor;
    let title = match tui.target() {
        Some(target) => format!("CONSULTA · {}", target.label()),
        None => "CONSULTA".to_string(),
    };
    pane_header(buf, area, &title, focused, tui);

    let rows = tui.areas.editor;
    tui.editor.scroll_into_view(rows.height as usize);
    let gutter = (tui.editor.line_count().to_string().len() + 2).max(4) as u16;
    let text_x = rows.x + gutter;
    let text_width = rows.width.saturating_sub(gutter + 1) as usize;
    // Rolagem horizontal: o cursor sempre cabe
    let hscroll = (tui.editor.cursor_x() + 1).saturating_sub(text_width);

    if tui.editor.is_empty() && !focused {
        let hint = format!(
            "escreva uma consulta ({}) · {} executa",
            tui.keys.first_key(Action::FocusEditor),
            tui.keys.first_key(Action::Run)
        );
        put(buf, text_x, rows.y, text_width as u16, &hint, tui.theme.subtle());
    }

    for line in 0..rows.height as usize {
        let row = tui.editor.scroll + line;
        let Some(text) = tui.editor.line(row) else {
            break;
        };
        let y = rows.y + line as u16;
        let number = format!("{:>width$} ", row + 1, width = gutter as usize - 2);
        put(buf, rows.x + 1, y, gutter - 1, &number, tui.theme.subtle());

        let chars: Vec<char> = text.chars().collect();
        let mut column = 0usize;
        for (start, end, token) in tui.editor.spans(row) {
            let style = match token {
                Token::Plain => tui.theme.text(),
                Token::Keyword => tui.theme.tone("accent_text").add_modifier(Modifier::BOLD),
                Token::String => tui.theme.tone("success"),
                Token::Number => tui.theme.tone("warning"),
                Token::Comment => tui.theme.subtle().add_modifier(Modifier::ITALIC),
            };
            for ch in &chars[start..end] {
                let width = ch.width().unwrap_or(0);
                if column >= hscroll && column + width <= hscroll + text_width {
                    let mut encoded = [0u8; 4];
                    put(buf, text_x + (column - hscroll) as u16, y, width as u16, ch.encode_utf8(&mut encoded), style);
                }
                column += width;
            }
        }
    }

    let cursor_line = tui.editor.row.checked_sub(tui.editor.scroll)?;
    (cursor_line < rows.height as usize).then(|| {
        Position::new(
            text_x + (tui.editor.cursor_x() - hscroll) as u16,
            rows.y + cursor_line as u16,
        )
    })
}

fn draw_completion(tui: &Tui, buf: &mut Buffer, cursor: Position, editor: Rect) {
    let Some(completion) = &tui.editor.completion else {
        return;
    };
    // Ícone, nome e, à direita, o detalhe (o tipo da coluna, o schema)
    let label = completion.items.iter().map(|item| item.label.width()).max().unwrap_or(0).clamp(8, 36);
    let detail = completion.items.iter().map(|item| item.detail.width()).max().unwrap_or(0).min(18);
    let width = (label + if detail > 0 { detail + 2 } else { 0 } + 4) as u16;
    let visible = completion.items.len().min(8);
    let x = cursor.x.min((editor.x + editor.width).saturating_sub(width));
    let first = completion.selected.saturating_sub(visible - 1);
    for (line, item) in completion.items.iter().enumerate().skip(first).take(visible) {
        let y = cursor.y + 1 + (line - first) as u16;
        if y >= buf.area().y + buf.area().height.saturating_sub(1) {
            break;
        }
        let style = if line == completion.selected { tui.theme.selection(true) } else { tui.theme.header(false) };
        let on_row = |tone: Style| style.patch(tone.bg(style.bg.unwrap_or_default()));
        fill(buf, Rect::new(x, y, width, 1), style);
        let glyph = match item.kind {
            ItemKind::Column => on_row(tui.theme.tone("accent_text")),
            ItemKind::Keyword => on_row(tui.theme.subtle()),
            _ => on_row(tui.theme.muted()),
        };
        put(buf, x + 1, y, 1, item.kind.glyph(), glyph);
        put(buf, x + 3, y, label as u16, &fit(&item.label, label), style);
        if detail > 0 {
            let text = fit(&item.detail, detail);
            put(buf, x + width - 1 - text.width() as u16, y, detail as u16, &text, on_row(tui.theme.subtle()));
        }
    }
}

fn draw_grid(tui: &mut Tui, buf: &mut Buffer, area: Rect) {
    let focused = tui.focus == Pane::Grid;
    let rows_area = tui.areas.grid;

    let Some(grid) = tui.grid.as_mut() else {
        pane_header(buf, area, "RESULTADOS", focused, tui);
        draw_empty(tui, buf, rows_area);
        return;
    };

    // Duas linhas de cabeçalho (nome e tipo); à esquerda, o número da linha
    let height = rows_area.height.saturating_sub(GRID_HEADER_LINES) as usize;
    let gutter = grid.gutter_width() as u16;
    grid.scroll_into_view(height, rows_area.width.saturating_sub(gutter + 1) as usize);

    let mut title = format!("RESULTADOS · {} linha(s)", grid.rows.len());
    if grid.has_more {
        title.push('+');
    }
    match grid.pending_count() {
        0 => {}
        pending => title.push_str(&format!(" · {pending} pendente(s)")),
    }
    let grid = tui.grid.as_ref().expect("grade presente");
    pane_header(buf, area, &title, focused, tui);

    if grid.is_empty() {
        put(buf, rows_area.x + 1, rows_area.y + 1, rows_area.width.saturating_sub(2), "sem colunas", tui.theme.muted());
        return;
    }

    let theme = &tui.theme;
    let end = rows_area.x + rows_area.width;
    // As colunas que cabem: (coluna, onde começa o texto, largura na tela)
    let mut visible: Vec<(usize, u16, usize)> = Vec::new();
    let mut x = rows_area.x + gutter + 2;
    for col in grid.scroll_col..grid.columns.len() {
        if x >= end {
            break;
        }
        let width = grid.width_of(col).min(end.saturating_sub(x) as usize);
        visible.push((col, x, width));
        x += (grid.width_of(col) + COLUMN_CHROME) as u16;
    }
    // O divisor fica uma coluna depois do texto de cada coluna (e do gutter)
    let dividers = |buf: &mut Buffer, y: u16, style: Style| {
        put(buf, rows_area.x + gutter, y, 1, "│", style);
        for (col, x, _) in &visible {
            let at = x + grid.width_of(*col) as u16 + 1;
            if at < end {
                put(buf, at, y, 1, "│", style);
            }
        }
    };
    let aligned = |text: String, width: usize, kind: CellKind| match kind {
        CellKind::Number => format!("{}{text}", " ".repeat(width.saturating_sub(text.width()))),
        _ => text,
    };

    // Cabeçalho: o nome e, embaixo, o tipo
    let header_style = theme.column_header();
    let type_style = header_style.patch(theme.subtle().bg(header_style.bg.unwrap_or_default())).remove_modifier(Modifier::BOLD);
    let divider_on = |style: Style| theme.divider().bg(style.bg.unwrap_or_default());
    fill(buf, Rect::new(rows_area.x, rows_area.y, rows_area.width, GRID_HEADER_LINES.min(rows_area.height)), header_style);
    put(buf, rows_area.x, rows_area.y, gutter, &format!("{:>width$} ", "#", width = gutter as usize - 1), type_style);
    for (col, x, width) in &visible {
        let kind = grid.kind_of(*col);
        put(buf, *x, rows_area.y, *width as u16, &aligned(fit(&grid.columns[*col], *width), *width, kind), header_style);
        let type_name = grid.types.get(*col).map(|name| name.to_lowercase()).unwrap_or_default();
        put(buf, *x, rows_area.y + 1, *width as u16, &aligned(fit(&type_name, *width), *width, kind), type_style);
    }
    for line in 0..GRID_HEADER_LINES.min(rows_area.height) {
        dividers(buf, rows_area.y + line, divider_on(header_style));
    }

    for line in 0..height {
        let row = grid.scroll_row + line;
        if row >= grid.rows.len() {
            break;
        }
        let y = rows_area.y + GRID_HEADER_LINES + line as u16;
        let current = row == grid.row;
        let row_state = grid.row_pending(row);
        // O fundo da linha: o da pendência (que a seleção nunca apaga) ou o
        // da linha atual
        let row_style = match (row_state, current) {
            (Some(kind), _) => theme.pending(kind),
            (None, true) => theme.selection(false),
            (None, false) => theme.text(),
        };
        if row_state.is_some() || current {
            fill(buf, Rect::new(rows_area.x, y, rows_area.width, 1), row_style);
        }

        // Número da linha, com o símbolo da pendência (cor e forma)
        let on_row = |style: Style| style.bg(row_style.bg.unwrap_or_default());
        let (marker, marker_style) = match grid.row_marker(row) {
            Some(Pending::Added) => ("+", on_row(theme.tone("success"))),
            Some(Pending::Removed) => ("−", on_row(theme.tone("danger"))),
            Some(Pending::Edited) => ("•", on_row(theme.tone("warning"))),
            None => (" ", row_style),
        };
        put(buf, rows_area.x, y, 1, marker, marker_style);
        let number_style = match current {
            true => on_row(theme.tone("accent_text")),
            false => on_row(theme.subtle()),
        };
        put(buf, rows_area.x + 1, y, gutter - 1, &format!("{:>width$}", row + 1, width = gutter as usize - 2), number_style);
        dividers(buf, y, divider_on(row_style));

        for (col, x, width) in &visible {
            let kind = grid.kind_of(*col);
            let value = grid.cell(row, *col);
            let edited = row_state.is_none() && grid.cell_pending(row, *col) == Some(Pending::Edited);
            let base = if edited { theme.pending(Pending::Edited) } else { row_style };
            let mut style = match (value, row_state) {
                // Linha removida: apagada e riscada
                (_, Some(Pending::Removed)) => base.patch(theme.subtle().bg(base.bg.unwrap_or_default())).add_modifier(Modifier::CROSSED_OUT),
                (None, _) => base.patch(theme.disabled().bg(base.bg.unwrap_or_default())),
                (Some(_), _) => base.patch(theme.cell(kind)),
            };
            if focused && current && *col == grid.col {
                // O cursor não esconde a pendência: mantém o fundo dela, em negrito
                style = match grid.cell_pending(row, *col) {
                    Some(_) => style.add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    None => theme.cell_cursor(),
                };
            }

            let text = match value {
                Some(value) => {
                    let value = shown(kind, value);
                    let first = value.lines().next().unwrap_or_default();
                    let text = if value.contains('\n') { format!("{first}⏎") } else { first.to_string() };
                    fit(&text, *width)
                }
                None => fit("NULL", *width),
            };
            fill(buf, Rect::new(*x, y, *width as u16, 1), style);
            put(buf, *x, y, *width as u16, &aligned(text, *width, kind), style);
        }
    }
}

/// A grade vazia: o polvo (se couber e estiver ligado) ao lado das
/// boas-vindas — a versão e os atalhos para começar.
fn draw_empty(tui: &Tui, buf: &mut Buffer, area: Rect) {
    let title = format!("octapus {}", env!("CARGO_PKG_VERSION"));
    let hints = [
        (tui.keys.first_key(Action::Open), "abre uma tabela na árvore"),
        (tui.keys.first_key(Action::Run), "executa a consulta do editor"),
        (tui.keys.first_key(Action::Search), "busca tabela ou schema"),
        (tui.keys.first_key(Action::Help), "todos os atalhos"),
    ];
    let key_width = hints.iter().map(|(key, _)| key.width()).max().unwrap_or(0) as u16;
    let text_width = hints.iter().map(|(_, label)| label.width()).max().unwrap_or(0) as u16 + key_width + 2;
    let text_height = hints.len() as u16 + 2;

    let (art_width, art_height) = super::mascot::size();
    let gap = 4;
    // Lado a lado se couber; senão, só o texto
    let with_art = tui.ui.mascot && area.width >= art_width + gap + text_width + 2 && area.height >= art_height.max(text_height);
    let block_width = if with_art { art_width + gap + text_width } else { text_width };
    let block_height = if with_art { art_height.max(text_height) } else { text_height };
    if area.width < text_width + 2 || area.height < text_height {
        return;
    }
    let left = area.x + (area.width - block_width) / 2;
    let top = area.y + (area.height - block_height) / 2;

    let mut text_x = left;
    if with_art {
        let art_top = top + (block_height - art_height) / 2;
        for (line, text) in super::mascot::art(tui.blink).iter().enumerate() {
            put(buf, left, art_top + line as u16, art_width, text, tui.theme.tone("accent_text"));
        }
        text_x += art_width + gap;
    }

    let text_top = top + (block_height - text_height) / 2;
    put(buf, text_x, text_top, text_width, &title, tui.theme.text().add_modifier(Modifier::BOLD));
    for (line, (key, label)) in hints.iter().enumerate() {
        let y = text_top + 2 + line as u16;
        put(buf, text_x, y, key_width, key, tui.theme.tone("accent_text"));
        put(buf, text_x + key_width + 2, y, text_width, label, tui.theme.subtle());
    }
}

fn draw_status(tui: &Tui, buf: &mut Buffer, area: Rect) {
    let chrome = tui.theme.chrome();
    fill(buf, area, chrome);

    let hints = match tui.focus {
        Pane::Tree => [Action::Open, Action::Filter, Action::NextPane, Action::Help],
        Pane::Editor => [Action::Run, Action::Complete, Action::NextPane, Action::Help],
        Pane::Grid => [Action::Edit, Action::Save, Action::NextPane, Action::Help],
    };
    let labels = |action: Action| match action {
        Action::Open => "abrir",
        Action::Filter => "filtrar",
        Action::NextPane => "painel",
        Action::Help => "ajuda",
        Action::Run => "executar",
        Action::Complete => "completar",
        Action::Edit => "editar",
        Action::Save => "gravar",
        _ => "",
    };
    let hints: Vec<(String, &str)> = hints.iter().map(|action| (tui.keys.first_key(*action), labels(*action))).collect();
    let right_width = hints.iter().map(|(key, label)| key.width() + label.width() + 1).sum::<usize>() as u16
        + 3 * (hints.len() as u16).saturating_sub(1);
    let right_x = area.x + area.width.saturating_sub(right_width + 1);
    if area.width > right_width + 20 {
        // A tecla em destaque, o que ela faz apagado
        let key_style = chrome.patch(tui.theme.tone("accent_text").bg(chrome.bg.unwrap_or_default())).add_modifier(Modifier::BOLD);
        let mut x = right_x;
        for (index, (key, label)) in hints.iter().enumerate() {
            if index > 0 {
                x = put(buf, x, area.y, 3, " · ", chrome.patch(tui.theme.divider().bg(chrome.bg.unwrap_or_default())));
            }
            x = put(buf, x, area.y, key.width() as u16, key, key_style);
            x = put(buf, x + 1, area.y, label.width() as u16, label, chrome);
        }
    }

    let mut x = area.x + 1;
    let room_end = if area.width > right_width + 20 { right_x.saturating_sub(2) } else { area.x + area.width };
    if tui.busy > 0 {
        let frame = SPINNER[(tui.phase / 0.35) as usize % SPINNER.len()];
        x = put(buf, x, area.y, 2, frame, chrome.patch(tui.theme.tone("accent_text").bg(chrome.bg.unwrap_or_default())));
        x += 1;
    }
    let (tone, message) = match &tui.status {
        Some((tone, message)) => (*tone, message.clone()),
        None => (Tone::Info, tui.target().map(|target| target.label()).unwrap_or_default()),
    };
    let token = match tone {
        Tone::Info => "muted",
        Tone::Ok => "success",
        Tone::Warn => "warning",
        Tone::Error => "danger",
    };
    let style = chrome.patch(tui.theme.tone(token).bg(chrome.bg.unwrap_or_default()));
    let room = room_end.saturating_sub(x) as usize;
    let message = message.replace('\n', " ");
    put(buf, x, area.y, room as u16, &fit(&message, room), style);
}

/// Um retângulo centrado de até `width` x `height`.
fn centered(area: Rect, width: u16, height: u16) -> Rect {
    let width = width.min(area.width.saturating_sub(2));
    let height = height.min(area.height.saturating_sub(2));
    Rect::new(
        area.x + (area.width - width) / 2,
        area.y + (area.height - height) / 2,
        width,
        height,
    )
}

fn draw_modal(tui: &Tui, frame: &mut Frame, modal: &Modal) {
    let area = frame.area();
    let style = tui.theme.header(false).remove_modifier(Modifier::BOLD);
    let block = |title: &str| {
        Block::bordered()
            .border_type(BorderType::Rounded)
            .title(format!(" {title} "))
            .style(style)
            .border_style(tui.theme.tone("accent_text").bg(style.bg.unwrap_or_default()))
    };

    match modal {
        Modal::Help => {
            let lines: Vec<String> = Action::ALL
                .iter()
                .map(|(action, _, _, description)| format!("{:<22} {description}", tui.keys.keys_of(*action).join(", ")))
                .collect();
            let rect = centered(area, 64, lines.len() as u16 + 2);
            frame.render_widget(Clear, rect);
            frame.render_widget(Paragraph::new(lines.join("\n")).block(block("Atalhos")), rect);
        }
        Modal::Cell { title, text } => {
            let rect = centered(area, area.width * 7 / 10, area.height * 6 / 10);
            frame.render_widget(Clear, rect);
            frame.render_widget(
                Paragraph::new(text.as_str()).wrap(Wrap { trim: false }).block(block(title)),
                rect,
            );
        }
        Modal::Confirm { lines, .. } => {
            let mut text = lines.join("\n");
            text.push_str("\n\n[s] sim    [qualquer outra tecla] não");
            let rect = centered(area, 72, lines.len() as u16 + 4);
            frame.render_widget(Clear, rect);
            frame.render_widget(Paragraph::new(text).wrap(Wrap { trim: false }).block(block("Confirmar")), rect);
        }
        Modal::Search { value, hits, selected } => {
            let rect = centered(area, 76, (hits.len() as u16).clamp(1, 14) + 4);
            frame.render_widget(Clear, rect);
            frame.render_widget(block("Buscar no catálogo"), rect);
            let inner = Rect::new(rect.x + 2, rect.y + 1, rect.width.saturating_sub(4), rect.height.saturating_sub(2));
            let buf = frame.buffer_mut();
            let end = put(buf, inner.x, inner.y, inner.width, &fit(value, inner.width as usize), style);
            let list = Rect::new(inner.x, inner.y + 2, inner.width, inner.height.saturating_sub(2));
            if hits.is_empty() {
                let hint = match value.is_empty() {
                    true => "digite um nome · schema. lista um schema · .tabela procura em todos",
                    false => "nada encontrado nos databases abertos",
                };
                put(buf, list.x, list.y, list.width, &fit(hint, list.width as usize), tui.theme.muted().bg(style.bg.unwrap_or_default()));
            }
            // Só as linhas que cabem, com a seleção sempre à vista
            let first = (selected + 1).saturating_sub(list.height as usize);
            for (line, hit) in hits.iter().enumerate().skip(first).take(list.height as usize) {
                let y = list.y + (line - first) as u16;
                let row = if line == *selected { tui.theme.selection(true) } else { style };
                fill(buf, Rect::new(list.x, y, list.width, 1), row);
                let glyph = match hit.kind {
                    CatalogKind::Schema => "◇",
                    CatalogKind::View | CatalogKind::MaterializedView => "◫",
                    CatalogKind::Foreign => "⇄",
                    CatalogKind::Table | CatalogKind::Partitioned => "▤",
                };
                let place = match (&hit.schema, hit.many) {
                    (_, Some(count)) => format!("{count} schemas · {}", hit.database),
                    (Some(schema), None) => format!("{schema} · {}", hit.database),
                    (None, None) => hit.database.clone(),
                };
                let place = fit(&place, (list.width / 2) as usize);
                let name_room = (list.width as usize).saturating_sub(place.width() + 4);
                put(buf, list.x, y, list.width, &format!("{glyph} {}", fit(&hit.name, name_room)), row);
                let x = list.x + list.width - place.width() as u16;
                put(buf, x, y, place.width() as u16, &place, tui.theme.muted().bg(row.bg.unwrap_or_default()));
            }
            frame.set_cursor_position(Position::new(end, inner.y));
        }
        Modal::Input { title, value, .. } => {
            let rect = centered(area, 60, 3);
            frame.render_widget(Clear, rect);
            let room = rect.width.saturating_sub(4) as usize;
            // Mostra o fim do texto quando ele não cabe
            let chars: Vec<char> = value.chars().collect();
            let mut shown = String::new();
            let mut used = 0;
            for ch in chars.iter().rev() {
                let width = ch.width().unwrap_or(0);
                if used + width > room {
                    break;
                }
                used += width;
                shown.insert(0, *ch);
            }
            frame.render_widget(Paragraph::new(format!(" {shown}")).block(block(title)), rect);
            frame.set_cursor_position(Position::new(rect.x + 2 + used as u16, rect.y + 1));
        }
    }
}
