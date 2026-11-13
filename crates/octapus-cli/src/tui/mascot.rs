//! O polvo da TUI: o logo do app em braille, com os tentáculos ondulando.
//!
//! O contorno vem de `mascot.txt`, um mapa de pontos rasterizado do próprio
//! logo (`scripts/mascot-bitmap.py`); cada célula do terminal mostra 2x4
//! pontos. A animação é só um deslocamento horizontal das linhas de baixo
//! (os tentáculos), em onda — nada é calculado fora do quadro que aparece, e
//! o relógio que a move só roda enquanto o polvo está na tela.

const BITMAPS: &str = include_str!("mascot.txt");

/// Um tamanho do polvo: as linhas de pontos (`true` = aceso).
pub struct Bitmap {
    width: usize,
    rows: Vec<Vec<bool>>,
}

impl Bitmap {
    /// O bloco `@name` do `mascot.txt`.
    pub fn load(name: &str) -> Option<Self> {
        let mut rows: Vec<Vec<bool>> = Vec::new();
        let mut inside = false;
        for line in BITMAPS.lines() {
            match line.strip_prefix('@') {
                Some(block) => {
                    if inside {
                        break;
                    }
                    inside = block == name;
                }
                None if inside => rows.push(line.chars().map(|ch| ch == '#').collect()),
                None => {}
            }
        }
        let width = rows.first()?.len();
        Some(Self { width, rows })
    }

    /// O maior polvo que cabe em `columns` x `lines` células, se algum cabe.
    pub fn fitting(columns: u16, lines: u16) -> Option<Self> {
        ["large", "small"]
            .into_iter()
            .filter_map(Self::load)
            .find(|bitmap| bitmap.columns() <= columns as usize && bitmap.lines() <= lines as usize)
    }

    /// Largura em células (2 pontos por célula, mais a folga da onda).
    pub fn columns(&self) -> usize {
        self.width.div_ceil(2) + 2
    }

    /// Altura em células (4 pontos por célula).
    pub fn lines(&self) -> usize {
        self.rows.len().div_ceil(4)
    }

    fn dot(&self, x: isize, y: usize, phase: f32) -> bool {
        let Some(row) = self.rows.get(y) else {
            return false;
        };
        // A cabeça (a metade de cima) fica parada; daí para baixo a onda
        // cresce, e os tentáculos balançam mais na ponta
        let start = self.rows.len() as f32 * 0.55;
        let depth = (y as f32 - start).max(0.0) / (self.rows.len() as f32 - start);
        let shift = (depth * 2.2 * (phase + y as f32 * 0.35).sin()).round() as isize;
        // 2 pontos de folga à esquerda (a onda vai para os dois lados)
        let source = x - 2 - shift;
        source >= 0 && row.get(source as usize).copied().unwrap_or(false)
    }

    /// As linhas do polvo em braille, na fase `phase` da onda (radianos).
    pub fn render(&self, phase: f32) -> Vec<String> {
        // A posição de cada ponto dentro da célula braille (U+2800)
        const DOTS: [[u32; 2]; 4] = [[0x01, 0x08], [0x02, 0x10], [0x04, 0x20], [0x40, 0x80]];
        (0..self.lines())
            .map(|line| {
                (0..self.columns())
                    .map(|column| {
                        let mut bits = 0;
                        for (dy, pair) in DOTS.iter().enumerate() {
                            for (dx, bit) in pair.iter().enumerate() {
                                if self.dot((column * 2 + dx) as isize, line * 4 + dy, phase) {
                                    bits |= bit;
                                }
                            }
                        }
                        char::from_u32(0x2800 + bits).unwrap_or(' ')
                    })
                    .collect()
            })
            .collect()
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn both_sizes_load_and_keep_the_shape() {
        for name in ["large", "small"] {
            let bitmap = Bitmap::load(name).unwrap_or_else(|| panic!("falta o bloco @{name}"));
            assert!(bitmap.rows.iter().all(|row| row.len() == bitmap.width), "{name}: linhas iguais");
            let lit = bitmap.rows.iter().flatten().filter(|dot| **dot).count();
            let total = bitmap.width * bitmap.rows.len();
            assert!(lit * 5 > total && lit * 5 < total * 4, "{name}: nem vazio nem cheio");
        }
        assert!(Bitmap::load("nope").is_none());
    }

    #[test]
    fn the_head_stays_still_and_the_tentacles_move() {
        let bitmap = Bitmap::load("large").unwrap();
        let (a, b) = (bitmap.render(0.0), bitmap.render(1.6));
        assert_eq!(a.len(), bitmap.lines());
        assert!(a.iter().all(|line| line.chars().count() == bitmap.columns()));
        let head = bitmap.lines() / 2;
        assert_eq!(a[..head], b[..head], "a cabeça não se mexe");
        assert_ne!(a[head..], b[head..], "os tentáculos ondulam");
    }

    #[test]
    fn picks_the_size_that_fits() {
        assert_eq!(Bitmap::fitting(80, 24).unwrap().lines(), Bitmap::load("large").unwrap().lines());
        assert_eq!(Bitmap::fitting(20, 9).unwrap().lines(), Bitmap::load("small").unwrap().lines());
        assert!(Bitmap::fitting(8, 3).is_none());
    }
}
