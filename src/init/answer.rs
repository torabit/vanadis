//! Reading what somebody typed at a prompt.
//!
//! `docs/init.md` decides the four answers: a name, several names, empty, and `-`.

use thiserror::Error;

use crate::token::TokenPath;

/// What one answer to `token name?` said.
#[derive(Debug, Clone, PartialEq, Eq)]
pub enum Answer {
    /// Nothing was typed: take the default, or skip when there is none.
    Default,
    /// `-`: skip, whether or not there is a default.
    Skip,
    /// One token, or several when the value means more than one thing.
    Tokens(Vec<TokenPath>),
}

/// An answer that is not a token name.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
#[error("`{text}` is not a token name: lowercase, digits, internal hyphens, `.` between them")]
pub struct AnswerError {
    /// What was typed.
    pub text: String,
}

/// Reads `text` as an answer.
///
/// A bare name is a `role` token, which is what makes `bg` the whole answer for the token
/// most files start with. A name with a `.` in it is the path as written, so a value that
/// belongs somewhere else is still reachable without a second question.
///
/// # Errors
///
/// Returns [`AnswerError`] naming the first word that is not a token name.
pub fn answer(text: &str) -> Result<Answer, AnswerError> {
    let text = text.trim();
    if text.is_empty() {
        return Ok(Answer::Default);
    }
    if text == "-" {
        return Ok(Answer::Skip);
    }
    text.split_whitespace()
        .map(|word| {
            let path = if word.contains('.') {
                TokenPath::parse(word)
            } else {
                TokenPath::parse("role").and_then(|role| role.child(word))
            };
            path.ok_or_else(|| AnswerError {
                text: word.to_owned(),
            })
        })
        .collect::<Result<Vec<_>, _>>()
        .map(Answer::Tokens)
}

#[cfg(test)]
mod tests {
    use super::*;

    fn paths(text: &str) -> Vec<String> {
        match answer(text) {
            Ok(Answer::Tokens(tokens)) => {
                tokens.iter().map(|path| path.as_str().to_owned()).collect()
            }
            other => panic!("expected tokens, got {other:?}"),
        }
    }

    #[test]
    fn reads_a_bare_name_as_a_role() {
        assert_eq!(paths("bg"), vec!["role.bg"]);
    }

    #[test]
    fn reads_a_qualified_name_as_written() {
        assert_eq!(paths("colors.paper"), vec!["colors.paper"]);
    }

    #[test]
    fn reads_a_nested_namespace() {
        assert_eq!(paths("role.git.added"), vec!["role.git.added"]);
    }

    #[test]
    fn reads_two_names_as_two_tokens() {
        assert_eq!(
            paths("comment inactive"),
            vec!["role.comment", "role.inactive"]
        );
    }

    #[test]
    fn reads_a_hyphenated_role() {
        assert_eq!(paths("accent-alt"), vec!["role.accent-alt"]);
    }

    #[test]
    fn reads_nothing_typed_as_the_default() {
        assert_eq!(answer(""), Ok(Answer::Default));
    }

    #[test]
    fn reads_whitespace_as_the_default() {
        assert_eq!(answer("  \t "), Ok(Answer::Default));
    }

    #[test]
    fn reads_a_hyphen_as_a_skip() {
        assert_eq!(answer("-"), Ok(Answer::Skip));
    }

    #[test]
    fn reports_a_name_that_is_not_a_segment() {
        assert_eq!(
            answer("added_bg"),
            Err(AnswerError {
                text: "added_bg".to_owned()
            })
        );
    }

    #[test]
    fn reports_the_word_that_is_wrong_rather_than_the_whole_line() {
        assert_eq!(
            answer("comment Inactive"),
            Err(AnswerError {
                text: "Inactive".to_owned()
            })
        );
    }
}
