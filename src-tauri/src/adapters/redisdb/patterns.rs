//! Os padrões de nomenclatura das chaves, para o contexto do assistente (B9).
//!
//! O Redis não tem estrutura declarada: o que o modelo recebe são os padrões
//! das chaves de uma amostra (`SCAN`, nunca `KEYS`) e o `TYPE` de cada um. A
//! amostra fica aqui dentro: **nenhuma chave completa nem valor sai** (RN-09)
//! — só o padrão, com cada parte que varia trocada por [`PLACEHOLDER`].
//!
//! A generalização falha fechado. As chaves se dividem nos `:` (a convenção
//! do Redis) e cada segmento só fica como está se tem cara de nome
//! (`user`, `cart`, `feature-flags`); número, UUID, hex, e-mail, data, token
//! e o que mais não parecer nome viram `{id}`, inteiros ou na parte que varia
//! (`user12345` → `user{id}`, `order_2024_001` → `order_{id}`). Depois, uma
//! posição com mais de [`MAX_LITERAL_SIBLINGS`] nomes diferentes sob o mesmo
//! prefixo também vira `{id}`: são valores (`city:porto-alegre`,
//! `city:recife`…), não estrutura.

use std::collections::{BTreeMap, HashMap};
use std::time::Duration;

use redis::aio::ConnectionManager;

use crate::error::{Error, Result};

use super::metadata::{scan_keys, SCAN_CAP};

/// O que entra no lugar de uma parte que varia. No `SCAN MATCH` vira `*`.
pub const PLACEHOLDER: &str = "{id}";

/// Chaves lidas pelo `SCAN` para montar os padrões. A ordem do `SCAN` segue
/// a tabela de hash do servidor, então as primeiras já são uma amostra
/// espalhada pelo keyspace.
pub const SAMPLE_KEYS: usize = 10_000;
const _: () = assert!(SAMPLE_KEYS <= SCAN_CAP);

/// Nomes diferentes numa mesma posição (sob o mesmo prefixo) acima disto
/// viram `{id}`.
pub const MAX_LITERAL_SIBLINGS: usize = 8;

/// Padrões com `TYPE` conferido, no máximo (os de mais chaves).
pub const MAX_PATTERNS: usize = 200;

/// Chaves de cada padrão cujo `TYPE` é lido.
const TYPE_SAMPLES: usize = 3;

/// Teto da amostragem inteira: o contexto não pode segurar o turno.
const SAMPLE_TIMEOUT: Duration = Duration::from_secs(5);

/// Segmento maior que isto não é nome.
const NAME_MAX_CHARS: usize = 40;

/// Um padrão de chave da amostra.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyPattern {
    /// O padrão generalizado (`user:{id}:cart`)
    pub pattern: String,
    /// Os `TYPE` vistos, o mais comum primeiro (`hash`, `zset`…)
    pub types: Vec<String>,
    /// Chaves da amostra com este padrão
    pub keys: usize,
}

/// Os padrões de um database, da amostra.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct KeyPatterns {
    /// Os mais frequentes primeiro (até [`MAX_PATTERNS`])
    pub patterns: Vec<KeyPattern>,
    /// Padrões da amostra que ficaram de fora do teto
    pub omitted: usize,
    /// Chaves lidas
    pub sampled: usize,
    /// `DBSIZE` (pode não estar disponível)
    pub total_keys: Option<u64>,
}

impl KeyPatterns {
    /// A amostra cobriu o database inteiro.
    pub fn complete(&self) -> bool {
        self.total_keys.is_some_and(|total| total as usize <= self.sampled)
    }
}

/// Amostra as chaves do database e devolve os padrões com o `TYPE` de cada
/// um. As chaves lidas morrem aqui.
pub async fn sample(conn: &mut ConnectionManager) -> Result<KeyPatterns> {
    let work = async {
        let total_keys: Option<u64> = redis::cmd("DBSIZE").query_async(conn).await.ok();
        let keys = scan_keys(conn, "*", SAMPLE_KEYS).await?;
        let groups = generalize(&keys);
        let omitted = groups.len().saturating_sub(MAX_PATTERNS);

        // TYPE de algumas chaves de cada padrão, num pipeline só
        let chosen = &groups[..groups.len().min(MAX_PATTERNS)];
        let mut pipe = redis::pipe();
        for group in chosen {
            for &index in group.members.iter().take(TYPE_SAMPLES) {
                pipe.cmd("TYPE").arg(&keys[index]);
            }
        }
        let types: Vec<String> = if chosen.is_empty() {
            Vec::new()
        } else {
            pipe.query_async(conn).await?
        };

        let mut types = types.into_iter();
        let patterns = chosen
            .iter()
            .map(|group| {
                let seen: Vec<String> = types.by_ref().take(group.members.len().min(TYPE_SAMPLES)).collect();
                KeyPattern {
                    pattern: group.pattern.clone(),
                    types: by_frequency(seen),
                    keys: group.members.len(),
                }
            })
            .collect();
        Ok::<_, Error>(KeyPatterns {
            patterns,
            omitted,
            sampled: keys.len(),
            total_keys,
        })
    };
    match tokio::time::timeout(SAMPLE_TIMEOUT, work).await {
        Ok(result) => result,
        Err(_) => Err(Error::Query("sampling the key patterns timed out".into())),
    }
}

/// Os tipos sem repetir, o mais comum primeiro (a chave que sumiu entre o
/// `SCAN` e o `TYPE` vem `none` e não conta).
fn by_frequency(types: Vec<String>) -> Vec<String> {
    let mut counts: Vec<(String, usize)> = Vec::new();
    for name in types.into_iter().filter(|name| name != "none") {
        match counts.iter_mut().find(|(seen, _)| *seen == name) {
            Some((_, count)) => *count += 1,
            None => counts.push((name, 1)),
        }
    }
    counts.sort_by_key(|(_, count)| std::cmp::Reverse(*count));
    counts.into_iter().map(|(name, _)| name).collect()
}

/// Um padrão e as chaves (índices na amostra) que ele cobre.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct PatternGroup {
    pub pattern: String,
    pub members: Vec<usize>,
}

/// Os padrões das chaves, os de mais chaves primeiro (empate: pelo nome).
pub fn generalize(keys: &[String]) -> Vec<PatternGroup> {
    let mut segmented: Vec<Vec<String>> = keys
        .iter()
        .map(|key| key.split(':').map(generalize_segment).collect())
        .collect();
    collapse_siblings(&mut segmented);

    let mut groups: HashMap<String, Vec<usize>> = HashMap::new();
    for (index, segments) in segmented.iter().enumerate() {
        groups.entry(segments.join(":")).or_default().push(index);
    }
    let mut groups: Vec<PatternGroup> = groups
        .into_iter()
        .map(|(pattern, members)| PatternGroup { pattern, members })
        .collect();
    groups.sort_by(|a, b| b.members.len().cmp(&a.members.len()).then_with(|| a.pattern.cmp(&b.pattern)));
    groups
}

/// Posição a posição, da esquerda para a direita: sob o mesmo prefixo (já
/// generalizado) e com o mesmo número de segmentos, mais de
/// [`MAX_LITERAL_SIBLINGS`] nomes diferentes viram `{id}`.
fn collapse_siblings(segmented: &mut [Vec<String>]) {
    let longest = segmented.iter().map(Vec::len).max().unwrap_or(0);
    for position in 0..longest {
        let mut literals: HashMap<(usize, String), BTreeMap<String, ()>> = HashMap::new();
        for segments in segmented.iter().filter(|segments| segments.len() > position) {
            let value = &segments[position];
            if value != PLACEHOLDER {
                let prefix = (segments.len(), segments[..position].join(":"));
                literals.entry(prefix).or_default().insert(value.clone(), ());
            }
        }
        for segments in segmented.iter_mut().filter(|segments| segments.len() > position) {
            let prefix = (segments.len(), segments[..position].join(":"));
            if literals.get(&prefix).is_some_and(|names| names.len() > MAX_LITERAL_SIBLINGS) {
                segments[position] = PLACEHOLDER.to_string();
            }
        }
    }
}

/// Um segmento (entre `:`) generalizado: fica se tem cara de nome; senão a
/// parte que varia vira `{id}`, ou o segmento inteiro.
pub fn generalize_segment(segment: &str) -> String {
    if segment.is_empty() || is_name(segment) {
        return segment.to_string();
    }
    if is_whole_id(segment) {
        return PLACEHOLDER.to_string();
    }

    // Por partes, nos separadores de dentro do segmento
    let mut out = String::new();
    let mut part = String::new();
    for c in segment.chars() {
        if matches!(c, '-' | '_' | '.') {
            out.push_str(&generalize_part(&part));
            out.push(c);
            part.clear();
        } else {
            part.push(c);
        }
    }
    out.push_str(&generalize_part(&part));

    let collapsed = collapse_placeholders(&out);
    if collapsed.chars().any(char::is_alphabetic) && collapsed != PLACEHOLDER {
        collapsed
    } else {
        PLACEHOLDER.to_string()
    }
}

/// Uma parte de segmento: nome, nome com número no fim (`user12345` →
/// `user{id}`) ou `{id}`.
fn generalize_part(part: &str) -> String {
    if part.is_empty() || is_name(part) {
        return part.to_string();
    }
    let digits = part.chars().rev().take_while(char::is_ascii_digit).count();
    let head: String = part.chars().take(part.chars().count() - digits).collect();
    // Só letras antes do número: `tok3n9x7` é token, não nome
    if digits > 0 && !head.is_empty() && head.chars().all(char::is_alphabetic) && is_name(&head) {
        return format!("{head}{PLACEHOLDER}");
    }
    PLACEHOLDER.to_string()
}

/// `{id}-{id}-{id}` (uma data, um UUID quebrado) vira `{id}`.
fn collapse_placeholders(text: &str) -> String {
    let mut text = text.to_string();
    loop {
        let before = text.len();
        for separator in ["-", "_", "."] {
            text = text.replace(&format!("{PLACEHOLDER}{separator}{PLACEHOLDER}"), PLACEHOLDER);
        }
        text = text.replace(&format!("{PLACEHOLDER}{PLACEHOLDER}"), PLACEHOLDER);
        if text.len() == before {
            return text;
        }
    }
}

/// Tem cara de nome: começa com letra, só letras, dígitos (no máximo dois),
/// `_` e `-`, curto, e não parece hex nem token de letras embaralhadas.
fn is_name(text: &str) -> bool {
    let chars: Vec<char> = text.chars().collect();
    let Some(first) = chars.first() else {
        return false;
    };
    let digits = chars.iter().filter(|c| c.is_ascii_digit()).count();
    let upper = chars.iter().filter(|c| c.is_uppercase()).count();
    let lower = chars.iter().filter(|c| c.is_lowercase()).count();
    first.is_alphabetic()
        && chars.len() <= NAME_MAX_CHARS
        && chars.iter().all(|&c| c.is_alphanumeric() || c == '_' || c == '-')
        && digits <= 2
        && !looks_hex(text)
        // `aZkQpLmNbVxY`: token, não camelCase
        && !(chars.len() >= 12 && lower > 0 && upper * 3 > chars.len())
}

/// Hex com algum dígito (`e29b`, `deadbeef01`): pedaço de hash ou de UUID.
fn looks_hex(text: &str) -> bool {
    text.chars().count() >= 4
        && text.chars().all(|c| c.is_ascii_hexdigit())
        && text.chars().any(|c| c.is_ascii_digit())
}

/// O segmento inteiro é um identificador ou um valor: número, UUID, hex,
/// e-mail, ou tem espaço ou caractere de glob.
fn is_whole_id(segment: &str) -> bool {
    let chars: Vec<char> = segment.chars().collect();
    let hex_without_dashes: String = chars.iter().filter(|&&c| c != '-').collect();
    chars.iter().all(char::is_ascii_digit)
        || (hex_without_dashes.len() == 32 && hex_without_dashes.chars().all(|c| c.is_ascii_hexdigit()))
        || looks_hex(segment)
        || chars.iter().any(|&c| c == '@' || c.is_whitespace() || c.is_control())
        || chars.iter().any(|c| matches!(c, '*' | '?' | '[' | ']' | '\\'))
}

/// O padrão como `MATCH` do `SCAN`: `{id}` vira `*` (a regra que o prompt
/// `redis-v1` ensina ao modelo).
#[cfg(test)]
pub fn match_pattern(pattern: &str) -> String {
    pattern.replace(PLACEHOLDER, "*")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn keys(list: &[&str]) -> Vec<String> {
        list.iter().map(|key| key.to_string()).collect()
    }

    fn patterns(list: &[String]) -> Vec<(String, usize)> {
        generalize(list).into_iter().map(|group| (group.pattern, group.members.len())).collect()
    }

    #[test]
    fn segments_that_vary_become_the_placeholder() {
        let cases = [
            ("user", "user"),
            ("feature-flags", "feature-flags"),
            ("v2", "v2"),
            ("42", "{id}"),
            ("550e8400-e29b-41d4-a716-446655440000", "{id}"),
            ("550e8400e29b41d4a716446655440000", "{id}"),
            ("66f1c0de0000000000000001", "{id}"),
            ("deadbeef01", "{id}"),
            ("ana@example.com", "{id}"),
            ("2026-01-31", "{id}"),
            ("2026-01-31T10:00:00Z", "{id}"),
            ("user12345", "user{id}"),
            ("order_2024_001", "order_{id}"),
            ("report-2026-01", "report-{id}"),
            ("aZkQpLmNbVxYtR", "{id}"),
            ("tok3n9x7", "{id}"),
            ("hello world", "{id}"),
            ("user*", "{id}"),
            ("getUserProfile", "getUserProfile"),
            ("", ""),
        ];
        for (segment, expected) in cases {
            assert_eq!(generalize_segment(segment), expected, "{segment}");
        }
    }

    #[test]
    fn groups_keys_by_pattern() {
        let sample = keys(&[
            "user:1",
            "user:2",
            "user:3",
            "user:1:cart",
            "user:2:cart",
            "session:550e8400-e29b-41d4-a716-446655440000",
            "session:6ba7b810-9dad-11d1-80b4-00c04fd430c8",
            "leaderboard",
            "queue:high",
            "queue:low",
        ]);
        assert_eq!(
            patterns(&sample),
            [
                ("user:{id}".to_string(), 3),
                ("session:{id}".to_string(), 2),
                ("user:{id}:cart".to_string(), 2),
                ("leaderboard".to_string(), 1),
                ("queue:high".to_string(), 1),
                ("queue:low".to_string(), 1),
            ]
        );
    }

    #[test]
    fn many_names_in_one_position_are_values() {
        let cities = [
            "porto-alegre", "recife", "manaus", "curitiba", "salvador", "natal", "belem", "goiania", "fortaleza",
        ];
        let mut sample: Vec<String> = cities.iter().map(|city| format!("city:{city}:weather")).collect();
        sample.extend(keys(&["status:pending", "status:paid"]));
        let found = patterns(&sample);
        assert!(found.contains(&("city:{id}:weather".to_string(), cities.len())), "{found:?}");
        assert!(found.contains(&("status:pending".to_string(), 1)));
    }

    #[test]
    fn match_pattern_uses_the_glob() {
        assert_eq!(match_pattern("user:{id}:cart"), "user:*:cart");
        assert_eq!(match_pattern("user{id}"), "user*");
    }

    /// RN-09: nenhuma chave da amostra (com cara de registro) aparece inteira
    /// num padrão, nem um pedaço que a identifique.
    #[test]
    fn no_full_key_or_identifier_leaves() {
        let sample = keys(&[
            "user:48213:profile",
            "user:48214:profile",
            "session:CANARYaZkQpLmNbVxY",
            "email:ana.canary@example.com",
            "token:7f3a9b1c2d4e5f60",
            "cart:550e8400-e29b-41d4-a716-446655440000",
            "order_2026_0042",
            "cpf:12345678901",
        ]);
        let found = patterns(&sample);
        for (pattern, _) in &found {
            assert!(!sample.contains(pattern), "chave completa saiu: {pattern}");
            for canary in ["48213", "CANARY", "canary", "example.com", "7f3a", "550e8400", "0042", "12345678901"] {
                assert!(!pattern.contains(canary), "{canary} vazou em {pattern}");
            }
        }
    }

    #[test]
    fn types_by_frequency_without_missing_keys() {
        let types = by_frequency(["hash", "string", "hash", "none"].map(String::from).to_vec());
        assert_eq!(types, ["hash", "string"]);
    }
}
