//! Template rendering.

use std::fmt;
use std::path::PathBuf;

use thiserror::Error;

use crate::token::{TokenPath, Tokens};

/// A template and the path it was read from.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Template {
    path: PathBuf,
    source: String,
}

/// One occurrence of a token the theme does not define.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct UndefinedToken {
    /// The token path, exactly as the template wrote it.
    pub token: TokenPath,
    /// 1-based line the occurrence starts on.
    pub line: usize,
}

/// Rendering failed.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum TemplateError {
    /// The template references tokens the theme does not define.
    #[error("{}: undefined tokens\n{}", .path.display(), listed(.occurrences))]
    UndefinedTokens {
        /// The template the occurrences were found in.
        path: PathBuf,
        /// Every occurrence, in source order.
        occurrences: Vec<UndefinedToken>,
    },
}

impl Template {
    /// A template with `source` as its body, attributed to `path` in errors.
    #[must_use]
    pub fn new(path: impl Into<PathBuf>, source: impl Into<String>) -> Self {
        Self {
            path: path.into(),
            source: source.into(),
        }
    }

    /// Substitutes every `{{token}}` and returns the rendered output.
    ///
    /// # Errors
    ///
    /// Returns every undefined token in one pass.
    pub fn render(&self, tokens: &Tokens) -> Result<String, TemplateError> {
        let mut output = String::with_capacity(self.source.len());
        let mut occurrences = Vec::new();
        let mut line = 1;
        let mut rest = self.source.as_str();

        while let Some(at) = rest.find(['{', '}']) {
            let (text, marker) = rest.split_at(at);
            output.push_str(text);
            line += text.matches('\n').count();

            rest = if let Some(tail) = marker.strip_prefix("{{{{") {
                output.push_str("{{");
                tail
            } else if let Some(tail) = marker.strip_prefix("}}}}") {
                output.push_str("}}");
                tail
            } else if let Some((token, tail)) = token_at(marker) {
                match tokens.get(&token) {
                    Some(value) => output.push_str(value),
                    None => occurrences.push(UndefinedToken { token, line }),
                }
                tail
            } else {
                // Braces that open nothing. The target tool's own syntax, left alone.
                output.push_str(&marker[..1]);
                &marker[1..]
            };
        }
        output.push_str(rest);

        if occurrences.is_empty() {
            Ok(output)
        } else {
            Err(TemplateError::UndefinedTokens {
                path: self.path.clone(),
                occurrences,
            })
        }
    }
}

/// Splits `{{token}}` off the front of `marker`, or `None` when it does not start one.
fn token_at(marker: &str) -> Option<(TokenPath, &str)> {
    let inner = marker.strip_prefix("{{")?;
    let end = inner.find("}}")?;
    let token = TokenPath::parse(&inner[..end])?;
    Some((token, &inner[end + 2..]))
}

impl fmt::Display for UndefinedToken {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        write!(f, "line {}: {}", self.line, self.token)
    }
}

/// The occurrences as one indented line each.
fn listed(occurrences: &[UndefinedToken]) -> String {
    occurrences
        .iter()
        .map(|occurrence| format!("  {occurrence}"))
        .collect::<Vec<_>>()
        .join("\n")
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(pairs: &[(&str, &str)]) -> Tokens {
        pairs
            .iter()
            .map(|(path, value)| (TokenPath::parse(path).unwrap(), (*value).to_owned()))
            .collect()
    }

    fn render(source: &str) -> String {
        Template::new("t.in", source)
            .render(&tokens(&[("role.bg", "#eeeeee"), ("ansi.0", "#000000")]))
            .unwrap()
    }

    fn undefined(source: &str) -> Vec<UndefinedToken> {
        match Template::new("t.in", source).render(&Tokens::default()) {
            Err(TemplateError::UndefinedTokens { occurrences, .. }) => occurrences,
            Ok(output) => panic!("expected undefined tokens, rendered {output:?}"),
        }
    }

    #[test]
    fn substitutes_a_defined_token() {
        assert_eq!(render("bg = \"{{role.bg}}\""), "bg = \"#eeeeee\"");
    }

    #[test]
    fn substitutes_every_occurrence() {
        assert_eq!(render("{{role.bg}}{{role.bg}}"), "#eeeeee#eeeeee");
    }

    #[test]
    fn passes_a_single_brace_through() {
        assert_eq!(render("{workspace} {tab}"), "{workspace} {tab}");
    }

    #[test]
    fn passes_a_spaced_name_through() {
        assert_eq!(
            render("content = \" {{ title }}\""),
            "content = \" {{ title }}\""
        );
    }

    #[test]
    fn passes_a_wildcard_through() {
        assert_eq!(render("{{role.*}} / {{ansi.*}}"), "{{role.*}} / {{ansi.*}}");
    }

    #[test]
    fn passes_an_unterminated_marker_through() {
        assert_eq!(render("{{role.bg"), "{{role.bg");
    }

    #[test]
    fn unescapes_doubled_braces() {
        assert_eq!(render("{{{{columns}}}}"), "{{columns}}");
    }

    #[test]
    fn leaves_multibyte_text_intact() {
        assert_eq!(
            render("# 配色は {{role.bg}} を参照する"),
            "# 配色は #eeeeee を参照する"
        );
    }

    #[test]
    fn reports_an_undefined_token() {
        assert_eq!(
            undefined("{{role.acent}}"),
            vec![UndefinedToken {
                token: TokenPath::parse("role.acent").unwrap(),
                line: 1
            }]
        );
    }

    #[test]
    fn reports_every_undefined_token_not_only_the_first() {
        let occurrences = undefined("{{role.a}}\n\n{{role.b}} {{role.c}}\n");
        let reported: Vec<_> = occurrences
            .iter()
            .map(|found| (found.token.as_str(), found.line))
            .collect();
        assert_eq!(reported, [("role.a", 1), ("role.b", 3), ("role.c", 3)]);
    }

    #[test]
    fn counts_lines_across_a_passed_through_marker() {
        let occurrences = undefined("{{ title }}\n{{role.a}}");
        assert_eq!(occurrences[0].line, 2);
    }

    #[test]
    fn names_the_template_in_the_error_message() {
        let error = Template::new("templates/rio/config.toml.in", "{{role.acent}}")
            .render(&Tokens::default())
            .unwrap_err();
        assert_eq!(
            error.to_string(),
            "templates/rio/config.toml.in: undefined tokens\n  line 1: role.acent"
        );
    }
}
