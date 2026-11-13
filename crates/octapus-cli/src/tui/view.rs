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
use super::editor::Token;
use octapus_core::models::NodeKind as CatalogKind;
use super::keys::Action;
use super::mascot::Bitmap;
use super::tree::Load;

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

fn pane_header(buf: &mut Buffer, area: Rect, title: &str, focused: bool, tui: &Tui) {
    let style = tui.theme.header(focused);
    fill(buf, Rect::new(area.x, area.y, area.width, 1), style);
    put(buf, area.x + 1, area.y, area.width.saturating_sub(2), title, style);
}

pub fn draw(tui: &mut Tui, frame: &mut Frame) {
    let area = frame.area();
    if area.width < 30 || area.height < 8 {
        frame.render_widget(Paragraph::new("terminal pequeno demais"), area);
        return;
    }
    fill(frame.buffer_mut(), area, tui.theme.text());

    let body_height = area.height - 1;
    let tree_width = (area.width * 3 / 10).clamp(22, 44).min(area.width / 2);
    let editor_height = (body_height * 35 / 100).clamp(4, 16);
    let right_x = area.x + tree_width + 1;
    let right_width = area.width - tree_width - 1;

    let tree = Rect::new(area.x, area.y, tree_width, body_height);
    let editor = Rect::new(right_x, area.y, right_width, editor_height);
    let grid = Rect::new(right_x, area.y + editor_height, right_width, body_height - editor_height);

    // As áreas de conteúdo (sem o cabeçalho de cada painel)
    let content = |rect: Rect| Rect::new(rect.x, rect.y + 1, rect.width, rect.height.saturating_sub(1));
    tui.areas = Areas {
        tree: content(tree),
        editor: content(editor),
        grid: content(grid),
    };

    let buf = frame.buffer_mut();
    for y in area.y..area.y + body_height {
        put(buf, area.x + tree_width, y, 1, "│", tui.theme.divider());
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
        let marker_style = match node.load {
            Load::Failed => tui.theme.tone("danger").bg(base.bg.unwrap_or_default()),
            _ => base.patch(tui.theme.subtle().bg(base.bg.unwrap_or_default())),
        };
        x = put(buf, x, y, end.saturating_sub(x), marker, marker_style);
        let glyph_style = base.patch(tui.theme.tone("accent_text").bg(base.bg.unwrap_or_default()));
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
    let width = completion.items.iter().map(|item| item.width()).max().unwrap_or(0).clamp(8, 40) as u16 + 2;
    let visible = completion.items.len().min(8);
    let x = cursor.x.min((editor.x + editor.width).saturating_sub(width));
    let first = completion.selected.saturating_sub(visible - 1);
    for (line, item) in completion.items.iter().enumerate().skip(first).take(visible) {
        let y = cursor.y + 1 + (line - first) as u16;
        if y >= buf.area().y + buf.area().height.saturating_sub(1) {
            break;
        }
        let style = if line == completion.selected { tui.theme.selection(true) } else { tui.theme.header(false) };
        fill(buf, Rect::new(x, y, width, 1), style);
        put(buf, x + 1, y, width - 2, item, style);
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

    // Uma linha para o cabeçalho das colunas
    let height = rows_area.height.saturating_sub(1) as usize;
    grid.scroll_into_view(height, rows_area.width.saturating_sub(1) as usize);

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

    let header_style = tui.theme.column_header();
    fill(buf, Rect::new(rows_area.x, rows_area.y, rows_area.width, 1), header_style);
    let end = rows_area.x + rows_area.width;

    let mut x = rows_area.x + 1;
    for col in grid.scroll_col..grid.columns.len() {
        if x >= end {
            break;
        }
        let width = grid.width_of(col).min(end.saturating_sub(x) as usize);
        put(buf, x, rows_area.y, width as u16, &fit(&grid.columns[col], width), header_style);

        for line in 0..height {
            let row = grid.scroll_row + line;
            if row >= grid.rows.len() {
                break;
            }
            let y = rows_area.y + 1 + line as u16;
            let value = grid.cell(row, col);
            let pending = grid.cell_pending(row, col);
            let is_cursor = focused && row == grid.row && col == grid.col;

            let mut style = match (pending, row == grid.row) {
                (Some(kind), _) => tui.theme.pending(kind),
                (None, true) => tui.theme.selection(false),
                (None, false) => tui.theme.text(),
            };
            if value.is_none() {
                style = style.patch(tui.theme.disabled().bg(style.bg.unwrap_or_default()));
            }
            if is_cursor {
                // O cursor não esconde a pendência: mantém o fundo dela, em negrito
                style = match pending {
                    Some(_) => style.add_modifier(Modifier::BOLD | Modifier::UNDERLINED),
                    None => tui.theme.cell_cursor(),
                };
            }

            // A linha inteira leva o fundo (pendência ou linha atual)
            if col == grid.scroll_col && (pending.is_some() || row == grid.row) {
                let row_style = match grid.row_pending(row) {
                    Some(kind) => tui.theme.pending(kind),
                    None if row == grid.row => tui.theme.selection(false),
                    None => tui.theme.text(),
                };
                fill(buf, Rect::new(rows_area.x, y, rows_area.width, 1), row_style);
            }

            let text = match value {
                Some(value) => {
                    let first = value.lines().next().unwrap_or_default();
                    let shown = if value.contains('\n') { format!("{first}⏎") } else { first.to_string() };
                    fit(&shown, width)
                }
                None => fit("NULL", width),
            };
            fill(buf, Rect::new(x, y, width as u16, 1), style);
            put(buf, x, y, width as u16, &text, style);
        }
        x += grid.width_of(col) as u16 + 2;
    }
}

/// A grade vazia: o polvo (se couber e estiver ligado) e as dicas.
fn draw_empty(tui: &Tui, buf: &mut Buffer, area: Rect) {
    let hints = [
        format!("{} abre uma tabela na árvore", tui.keys.first_key(Action::Open)),
        format!("{} executa a consulta do editor", tui.keys.first_key(Action::Run)),
        format!("{} ajuda", tui.keys.first_key(Action::Help)),
    ];
    let center = |text: &str| area.x + area.width.saturating_sub(text.width() as u16) / 2;

    let mascot = tui
        .ui
        .mascot
        .then(|| Bitmap::fitting(area.width.saturating_sub(2), area.height.saturating_sub(hints.len() as u16 + 2)))
        .flatten();
    let art_lines = mascot.as_ref().map_or(0, Bitmap::lines) as u16;
    let total = art_lines + hints.len() as u16 + u16::from(art_lines > 0);
    let mut y = area.y + area.height.saturating_sub(total) / 2;

    if let Some(mascot) = mascot {
        for line in mascot.render(tui.phase) {
            put(buf, center(&line), y, area.width, &line, tui.theme.tone("accent_text"));
            y += 1;
        }
        y += 1;
    }
    for hint in &hints {
        if y >= area.y + area.height {
            break;
        }
        put(buf, center(hint), y, area.width, hint, tui.theme.subtle());
        y += 1;
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
    let right: String = hints
        .iter()
        .map(|action| format!("{} {}", tui.keys.first_key(*action), labels(*action)))
        .collect::<Vec<_>>()
        .join("  ");
    let right_width = right.width() as u16;
    let right_x = area.x + area.width.saturating_sub(right_width + 1);
    if area.width > right_width + 20 {
        put(buf, right_x, area.y, right_width, &right, chrome);
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
