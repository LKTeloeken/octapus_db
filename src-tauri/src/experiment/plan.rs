//! As unidades de teste de uma rodada e a ordem delas.
//!
//! Uma unidade é (pergunta, idioma, modelo, paradigma, repetição). A ordem é
//! em blocos: cada bloco é uma pergunta numa repetição, com todas as condições
//! dela (paradigma × modelo × idioma) embaralhadas; os blocos de uma repetição
//! também são embaralhados, e as repetições vão uma depois da outra. Assim
//! nenhum modelo ou idioma roda inteiro antes do outro, e uma rodada
//! interrompida fica com repetições completas no começo.
//!
//! O embaralhamento usa um gerador próprio ([`Rng`], SplitMix64), para a mesma
//! semente dar a mesma ordem em qualquer máquina e versão.

use serde::{Deserialize, Serialize};

use super::questions::{Lang, QuestionSet};

#[derive(Debug, Clone, PartialEq, Eq, Hash, Serialize, Deserialize)]
#[serde(rename_all = "camelCase")]
pub struct Unit {
    pub question_id: String,
    pub paradigm: String,
    pub language: Lang,
    /// O id do modelo na API
    pub model: String,
    /// A partir de 1
    pub repetition: u32,
}

impl Unit {
    /// O que identifica a unidade na rodada (a retomada pula as já gravadas).
    pub fn key(&self) -> String {
        format!(
            "{}|{}|{}|{}|r{}",
            self.question_id,
            self.paradigm,
            self.language.code(),
            self.model,
            self.repetition
        )
    }
}

/// O que define as unidades de uma rodada.
pub struct Design<'a> {
    pub questions: &'a QuestionSet,
    /// Só estas perguntas (`--only`); vazio = todas
    pub only: &'a [String],
    /// Os paradigmas com banco no ambiente
    pub paradigms: &'a [String],
    pub languages: &'a [Lang],
    pub models: &'a [String],
    pub repetitions: u32,
    pub seed: u64,
}

/// As unidades na ordem em que rodam.
pub fn units(design: &Design<'_>) -> Vec<Unit> {
    let mut rng = Rng::new(design.seed);
    let mut ordered = Vec::new();

    for repetition in 1..=design.repetitions {
        let mut blocks: Vec<Vec<Unit>> = Vec::new();
        for question in &design.questions.questions {
            if !design.only.is_empty() && !design.only.contains(&question.id) {
                continue;
            }
            let mut block = Vec::new();
            for paradigm in design.paradigms.iter().filter(|name| question.runs_on(name)) {
                for model in design.models {
                    for language in design.languages {
                        block.push(Unit {
                            question_id: question.id.clone(),
                            paradigm: paradigm.clone(),
                            language: *language,
                            model: model.clone(),
                            repetition,
                        });
                    }
                }
            }
            rng.shuffle(&mut block);
            blocks.push(block);
        }
        rng.shuffle(&mut blocks);
        ordered.extend(blocks.into_iter().flatten());
    }
    ordered
}

/// SplitMix64: pequeno, sem dependência e com a sequência fixa para sempre.
struct Rng(u64);

impl Rng {
    fn new(seed: u64) -> Self {
        Self(seed)
    }

    fn next(&mut self) -> u64 {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        z ^ (z >> 31)
    }

    /// Fisher–Yates.
    fn shuffle<T>(&mut self, items: &mut [T]) {
        for index in (1..items.len()).rev() {
            let other = (self.next() % (index as u64 + 1)) as usize;
            items.swap(index, other);
        }
    }
}

#[cfg(test)]
mod tests {
    use std::collections::HashSet;

    use super::*;
    use crate::experiment::questions;

    fn set() -> QuestionSet {
        let mut source = String::new();
        for index in 1..=4 {
            source.push_str(&format!(
                "[[question]]\nid = \"q{index}\"\ndifficulty = \"easy\"\nskip_language_check = true\n\
                 pt = \"pergunta {index}\"\nen = \"question {index}\"\n\
                 [question.gold.postgres]\nquery = \"SELECT {index}\"\n"
            ));
            // A q1 também tem gabarito de SQLite
            if index == 1 {
                source.push_str("[question.gold.sqlite]\nquery = \"SELECT 1\"\n");
            }
        }
        source.push_str(
            "[[question]]\nid = \"sem\"\ndifficulty = \"hard\"\nskip_language_check = true\n\
             pt = \"sem resposta\"\nen = \"no answer\"\nunanswerable = true\n",
        );
        questions::load(&source).unwrap()
    }

    fn design<'a>(questions: &'a QuestionSet, paradigms: &'a [String], models: &'a [String], seed: u64) -> Design<'a> {
        Design {
            questions,
            only: &[],
            paradigms,
            languages: &[Lang::Pt, Lang::En],
            models,
            repetitions: 2,
            seed,
        }
    }

    #[test]
    fn covers_every_condition_once_per_repetition() {
        let questions = set();
        let paradigms = ["postgres".to_string(), "sqlite".to_string()];
        let models = ["a".to_string(), "b".to_string(), "c".to_string()];
        let units = units(&design(&questions, &paradigms, &models, 7));

        // q1: 2 paradigmas; q2..q4: só postgres; sem resposta: os 2
        let per_repetition = (2 + 3 + 2) * 3 * 2;
        assert_eq!(units.len(), per_repetition * 2);
        let keys: HashSet<String> = units.iter().map(Unit::key).collect();
        assert_eq!(keys.len(), units.len(), "nenhuma unidade repetida");

        // As repetições vão em sequência
        assert!(units[..per_repetition].iter().all(|unit| unit.repetition == 1));
        assert!(units[per_repetition..].iter().all(|unit| unit.repetition == 2));
        // Cada bloco é uma pergunta inteira
        let first = &units[0].question_id;
        let block = units.iter().take_while(|unit| &unit.question_id == first).count();
        assert_eq!(block, units.iter().filter(|unit| &unit.question_id == first && unit.repetition == 1).count());
    }

    #[test]
    fn same_seed_same_order_and_models_interleave() {
        let questions = set();
        let paradigms = ["postgres".to_string()];
        let models = ["a".to_string(), "b".to_string()];
        let order = |seed| -> Vec<String> {
            units(&design(&questions, &paradigms, &models, seed)).iter().map(Unit::key).collect()
        };
        assert_eq!(order(42), order(42));
        assert_ne!(order(42), order(43));

        // Nenhum modelo nem idioma roda inteiro antes do outro
        let units = units(&design(&questions, &paradigms, &models, 42));
        let half = units.len() / 2;
        for model in &models {
            assert!(units[..half].iter().any(|unit| &unit.model == model));
        }
        for language in [Lang::Pt, Lang::En] {
            assert!(units[..half].iter().any(|unit| unit.language == language));
        }
    }

    #[test]
    fn only_restricts_the_questions() {
        let questions = set();
        let paradigms = ["postgres".to_string()];
        let models = ["a".to_string()];
        let only = ["q2".to_string(), "sem".to_string()];
        let mut design = design(&questions, &paradigms, &models, 1);
        design.only = &only;
        design.repetitions = 1;
        let units = units(&design);
        assert_eq!(units.len(), 4);
        assert!(units.iter().all(|unit| unit.question_id == "q2" || unit.question_id == "sem"));
    }

    /// A sequência do gerador não pode mudar: mudaria a ordem de rodadas
    /// já registradas com a mesma semente.
    #[test]
    fn generator_sequence_is_pinned() {
        let mut rng = Rng::new(0);
        assert_eq!(rng.next(), 0xe220_a839_7b1d_cdaf);
        assert_eq!(rng.next(), 0x6e78_9e6a_a1b9_65f4);
        let mut items = [1, 2, 3, 4, 5, 6];
        Rng::new(42).shuffle(&mut items);
        let mut sorted = items;
        sorted.sort_unstable();
        assert_eq!(sorted, [1, 2, 3, 4, 5, 6]);
    }
}
