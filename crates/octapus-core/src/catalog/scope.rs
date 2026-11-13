//! Escopo salvo por conexão (refactor do catálogo, Fase 6): que databases e
//! schemas o app enxerga. Num servidor com 5.000 tenants, quem só cuida de
//! alguns diz quais (`tenant_00*`), e quem não quer ver os de teste os tira
//! (`!tenant_test_*`). O que fica de fora não é lido, listado nem buscado.
//!
//! Sintaxe: padrões separados por vírgula ou quebra de linha; `*` casa
//! qualquer trecho e `?` um caractere; `!` na frente exclui. Sem nenhum padrão
//! de inclusão, tudo entra (menos o excluído). Sem diferenciar maiúsculas.

#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct NameScope {
    include: Vec<Vec<char>>,
    exclude: Vec<Vec<char>>,
}

impl NameScope {
    /// `None` (ou só espaços) = sem restrição.
    pub fn parse(rules: Option<&str>) -> Self {
        let mut scope = Self::default();
        for raw in rules.unwrap_or_default().split([',', '\n']) {
            let rule = raw.trim();
            let (target, pattern) = match rule.strip_prefix('!') {
                Some(rest) => (&mut scope.exclude, rest.trim()),
                None => (&mut scope.include, rule),
            };
            if !pattern.is_empty() {
                target.push(pattern.to_lowercase().chars().collect());
            }
        }
        scope
    }

    pub fn is_unrestricted(&self) -> bool {
        self.include.is_empty() && self.exclude.is_empty()
    }

    pub fn allows(&self, name: &str) -> bool {
        if self.is_unrestricted() {
            return true;
        }
        let name: Vec<char> = name.to_lowercase().chars().collect();
        let included = self.include.is_empty() || self.include.iter().any(|pattern| glob(pattern, &name));
        included && !self.exclude.iter().any(|pattern| glob(pattern, &name))
    }
}

/// `*` e `?` sem backtracking exponencial: volta só até a última estrela.
fn glob(pattern: &[char], name: &[char]) -> bool {
    let (mut p, mut n) = (0, 0);
    let mut star: Option<(usize, usize)> = None;
    while n < name.len() {
        match pattern.get(p) {
            Some('*') => {
                star = Some((p, n));
                p += 1;
            }
            Some(&c) if c == '?' || c == name[n] => {
                p += 1;
                n += 1;
            }
            _ => match star {
                Some((star_p, star_n)) => {
                    p = star_p + 1;
                    n = star_n + 1;
                    star = Some((star_p, star_n + 1));
                }
                None => return false,
            },
        }
    }
    pattern[p..].iter().all(|&c| c == '*')
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn empty_rules_allow_everything() {
        for rules in [None, Some(""), Some(" , \n ")] {
            let scope = NameScope::parse(rules);
            assert!(scope.is_unrestricted());
            assert!(scope.allows("qualquer"));
        }
    }

    #[test]
    fn include_and_exclude_with_globs() {
        let scope = NameScope::parse(Some("tenant_*, public\n!tenant_test*, !*_old"));
        assert!(scope.allows("tenant_00042"));
        assert!(scope.allows("PUBLIC"));
        assert!(!scope.allows("tenant_test_01"));
        assert!(!scope.allows("tenant_42_old"));
        assert!(!scope.allows("audit"));
    }

    #[test]
    fn only_excludes_keep_the_rest() {
        let scope = NameScope::parse(Some("!pg_temp*, ! staging"));
        assert!(scope.allows("tenant_1"));
        assert!(!scope.allows("pg_temp_3"));
        assert!(!scope.allows("staging"));
    }

    #[test]
    fn glob_matches_like_a_shell() {
        let matches = |pattern: &str, name: &str| {
            glob(&pattern.chars().collect::<Vec<_>>(), &name.chars().collect::<Vec<_>>())
        };
        assert!(matches("t?nant_*", "tenant_1"));
        assert!(matches("*", ""));
        assert!(matches("a*b*c", "aXXbYYc"));
        assert!(matches("a*b*c", "abcbc"));
        assert!(!matches("a*b*c", "aXXbYY"));
        assert!(!matches("tenant", "tenant_1"));
        assert!(matches("**x", "abcx"));
        // Sem explosão em padrões patológicos
        let long = "a".repeat(5_000);
        assert!(!matches(&format!("{}b", "*a".repeat(50)), &long));
    }
}
