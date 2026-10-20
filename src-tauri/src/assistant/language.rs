//! O idioma da pergunta (C-02), gravado em cada turno (`Language` do
//! contrato). Serve à análise do experimento; o modelo responde no idioma da
//! pergunta por conta própria (o prompt pede).

use serde::{Deserialize, Serialize};
use whatlang::Lang;

#[cfg_attr(test, derive(ts_rs::TS), ts(export, rename = "AssistantLanguage"))]
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum Language {
    Pt,
    En,
    /// Qualquer outro (e valor novo da API)
    Other,
}

crate::models::lenient_deserialize!(Language { Pt, En } else Other);

/// Português, inglês ou outro, pelo `whatlang`. Pergunta curta demais para
/// decidir cai no palpite do detector; um idioma que não é nenhum dos dois
/// vira `Other`.
pub fn detect(question: &str) -> Language {
    match whatlang::detect_lang(question) {
        Some(Lang::Por) => Language::Pt,
        Some(Lang::Eng) => Language::En,
        _ => Language::Other,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_portuguese_english_and_others() {
        assert_eq!(detect("Quais clientes compraram mais no último mês?"), Language::Pt);
        assert_eq!(detect("quantos pedidos foram cancelados por cada vendedor"), Language::Pt);
        assert_eq!(detect("Which customers bought the most last month?"), Language::En);
        assert_eq!(detect("¿Cuántos pedidos se cancelaron la semana pasada?"), Language::Other);
        assert_eq!(detect(""), Language::Other);
        assert_eq!(
            serde_json::to_value(Language::Pt).unwrap(),
            serde_json::json!("pt")
        );
    }
}
