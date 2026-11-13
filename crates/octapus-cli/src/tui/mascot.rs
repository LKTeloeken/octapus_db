//! O polvo da TUI: o logo do app em caracteres de bloco, pequeno e quase
//! parado — só pisca de vez em quando.
//!
//! Os quadros vêm de `mascot.txt`, desenhados do próprio logo pelo
//! `scripts/mascot-bitmap.py` (cada célula mostra 2x2 pontos). Não há relógio
//! de animação: o laço da TUI acorda só na hora de abrir ou fechar o olho
//! ([`BLINK_EVERY`], [`BLINK_FOR`]), então a tela parada não gasta nada.

use std::time::Duration;

use unicode_width::UnicodeWidthStr;

const FRAMES: &str = include_str!("mascot.txt");

/// De quanto em quanto tempo o polvo pisca, e por quanto tempo.
pub const BLINK_EVERY: Duration = Duration::from_secs(5);
pub const BLINK_FOR: Duration = Duration::from_millis(180);

/// As linhas do quadro `@name` do `mascot.txt`.
fn frame(name: &str) -> Vec<&'static str> {
    let mut lines = Vec::new();
    let mut inside = false;
    for line in FRAMES.lines() {
        match line.strip_prefix('@') {
            Some(block) => {
                if inside {
                    break;
                }
                inside = block == name;
            }
            None if inside => lines.push(line),
            None => {}
        }
    }
    lines
}

/// O polvo: de olho aberto ou, em `blink`, piscando.
pub fn art(blink: bool) -> Vec<&'static str> {
    frame(if blink { "blink" } else { "open" })
}

/// Largura e altura em células.
pub fn size() -> (u16, u16) {
    let lines = art(false);
    (lines.iter().map(|line| line.width()).max().unwrap_or(0) as u16, lines.len() as u16)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_frames_have_the_same_shape_and_only_the_eyes_change() {
        let (open, blink) = (art(false), art(true));
        assert!(open.len() >= 5, "o polvo tem corpo e tentáculos");
        assert_eq!(open.len(), blink.len());
        assert_ne!(open, blink, "a piscada fecha o olho");
        // Os tentáculos (a metade de baixo) não se mexem
        let legs = open.len() / 2 + 1;
        assert_eq!(open[legs..], blink[legs..]);
        let (width, height) = size();
        assert!(width <= 20 && height <= 8, "pequeno: {width}x{height}");
    }
}
