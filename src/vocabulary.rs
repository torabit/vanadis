//! The tokens every theme is expected to define.
//!
//! `docs/core-vocabulary.md` decides the list and decides that an incomplete theme is not a
//! load error. The list is compiled in: a theme cannot extend it and nothing reads it from a
//! file.

use crate::token::{TokenPath, Tokens};

/// The seventeen `[role]` tokens, in the order `docs/core-vocabulary.md` tabulates them.
const ROLES: [&str; 17] = [
    "bg",
    "fg",
    "comment",
    "keyword",
    "string",
    "error",
    "ok",
    "warn",
    "visual",
    "linenr",
    "accent",
    "accent-alt",
    "accent-warm",
    "inactive",
    "border",
    "selection-bg",
    "hover-bg",
];

/// The core vocabulary: `[role]` then `[ansi]`.
///
/// `[meta]` is left out. `format`, `name` and `variant` are required for a theme to load at
/// all, so a theme that can be asked this question already has them.
#[must_use]
pub fn core() -> Vec<TokenPath> {
    let roles = ROLES
        .iter()
        .map(|role| TokenPath::from_segments(["role", role]));
    let slots = (0..16).map(|slot| TokenPath::from_segments(["ansi", &slot.to_string()]));
    roles.chain(slots).collect()
}

/// The core tokens `tokens` does not define, in core order.
#[must_use]
pub fn missing(tokens: &Tokens) -> Vec<TokenPath> {
    core()
        .into_iter()
        .filter(|path| tokens.get(path).is_none())
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(paths: &[&str]) -> Tokens {
        paths
            .iter()
            .map(|path| (TokenPath::parse(path).unwrap(), "#eeeeee".to_owned()))
            .collect()
    }

    #[test]
    fn counts_thirty_three_core_tokens() {
        assert_eq!(core().len(), 33);
    }

    #[test]
    fn starts_at_the_first_role() {
        assert_eq!(core()[0].as_str(), "role.bg");
    }

    #[test]
    fn ends_at_the_last_ansi_slot() {
        assert_eq!(core()[32].as_str(), "ansi.15");
    }

    #[test]
    fn spells_a_hyphenated_role() {
        assert!(core().iter().any(|path| path.as_str() == "role.accent-alt"));
    }

    #[test]
    fn calls_every_core_token_missing_from_an_empty_theme() {
        assert_eq!(missing(&Tokens::default()).len(), 33);
    }

    #[test]
    fn leaves_a_defined_token_out_of_the_missing_list() {
        let missing = missing(&tokens(&["role.bg"]));
        assert!(!missing.iter().any(|path| path.as_str() == "role.bg"));
        assert_eq!(missing.len(), 32);
    }

    #[test]
    fn ignores_a_token_outside_the_core() {
        assert_eq!(missing(&tokens(&["colors.paper"])).len(), 33);
    }
}
