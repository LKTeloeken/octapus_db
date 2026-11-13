//! O idioma da pergunta (C-02), gravado em cada turno (`Language` do
//! contrato) com a confiança do detector. Serve à análise do experimento; o
//! modelo responde no idioma da pergunta por conta própria (o prompt pede).

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

/// O idioma detectado e a confiança do detector no palpite (0 a 1).
#[derive(Debug, Clone, Copy, PartialEq)]
pub struct Detected {
    pub language: Language,
    pub confidence: f64,
}

/// Português, inglês ou outro, pelo `whatlang`. Pergunta curta demais para
/// decidir cai no palpite do detector, com confiança baixa — ela vai para a
/// API junto, para a análise separar esses turnos; um idioma que não é
/// nenhum dos dois vira `Other`.
pub fn detect(question: &str) -> Detected {
    let Some(info) = whatlang::detect(question) else {
        return Detected {
            language: Language::Other,
            confidence: 0.0,
        };
    };
    let language = match info.lang() {
        Lang::Por => Language::Pt,
        Lang::Eng => Language::En,
        _ => Language::Other,
    };
    Detected {
        language,
        confidence: info.confidence().clamp(0.0, 1.0),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn detects_portuguese_english_and_others() {
        let language = |question| detect(question).language;
        assert_eq!(language("Quais clientes compraram mais no último mês?"), Language::Pt);
        assert_eq!(language("quantos pedidos foram cancelados por cada vendedor"), Language::Pt);
        assert_eq!(language("Which customers bought the most last month?"), Language::En);
        assert_eq!(language("¿Cuántos pedidos se cancelaron la semana pasada?"), Language::Other);
        assert_eq!(detect(""), Detected { language: Language::Other, confidence: 0.0 });
        assert_eq!(
            serde_json::to_value(Language::Pt).unwrap(),
            serde_json::json!("pt")
        );
    }

    /// A confiança separa a pergunta longa da curta, em que o detector chuta.
    #[test]
    fn short_questions_come_with_low_confidence() {
        let long = detect("Quais clientes compraram mais no último mês, por cidade e por vendedor?");
        let short = detect("total vendas");
        assert!((0.0..=1.0).contains(&long.confidence) && (0.0..=1.0).contains(&short.confidence));
        assert!(long.confidence > short.confidence, "{long:?} {short:?}");
    }
}
