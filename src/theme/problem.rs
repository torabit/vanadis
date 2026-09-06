//! What can be wrong with a theme file.

use thiserror::Error;

use crate::token::TokenPath;

/// One thing wrong with a theme file.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum Problem {
    /// A key that is not a valid token segment.
    #[error("line {line}: key `{key}` is not a token segment: lowercase, digits, internal hyphens")]
    Key {
        /// The key, as the file writes it.
        key: String,
        /// The line the key is written on.
        line: usize,
    },
    /// A key under `[ansi]` that does not name one of the sixteen slots.
    #[error("line {line}: `{key}` under [ansi] is not a slot: the slots are `0` through `15`")]
    AnsiSlot {
        /// The key, as the file writes it.
        key: String,
        /// The line the key is written on.
        line: usize,
    },
    /// A value that is not a string.
    #[error("line {line}: {path} is a {found}, and every theme value is a string")]
    Type {
        /// The token holding the value.
        path: TokenPath,
        /// The line the token is written on.
        line: usize,
        /// The TOML type found instead.
        found: &'static str,
    },
    /// A colour value that is neither a hex literal nor a reference.
    #[error("line {line}: {path} is `{value}`, and a hex literal is `#` and six hex digits")]
    Hex {
        /// The token holding the value.
        path: TokenPath,
        /// The line the token is written on.
        line: usize,
        /// The value, as the file writes it.
        value: String,
    },
    /// A value that wraps a reference in other text.
    #[error("line {line}: {path} is `{value}`, and a reference must fill the whole value")]
    Reference {
        /// The token holding the value.
        path: TokenPath,
        /// The line the token is written on.
        line: usize,
        /// The value, as the file writes it.
        value: String,
    },
    /// A `[meta]` key the format does not define.
    #[error("line {line}: [meta] has no key `{key}`")]
    MetaUnknown {
        /// The key, as the file writes it.
        key: String,
        /// The line the key is written on.
        line: usize,
    },
    /// A required `[meta]` key the file does not carry.
    #[error("[meta] is missing `{key}`")]
    MetaMissing {
        /// The missing key.
        key: String,
    },
    /// A `meta.variant` that is neither `dark` nor `light`.
    #[error("line {line}: meta.variant is `{value}`, and a variant is `dark` or `light`")]
    MetaVariant {
        /// The line the key is written on.
        line: usize,
        /// The value, as the file writes it.
        value: String,
    },
    /// A `meta.format` this version does not read.
    #[error("line {line}: meta.format is `{found}`, and this version of vanadis reads format 1")]
    MetaFormat {
        /// The line the key is written on.
        line: usize,
        /// The value, or the TOML type when it is not a number.
        found: String,
    },
    /// A reference naming a token the theme does not define.
    #[error("line {line}: {path} references undefined {target}")]
    Undefined {
        /// The token holding the reference.
        path: TokenPath,
        /// The line the token is written on.
        line: usize,
        /// The path the reference names.
        target: TokenPath,
    },
    /// A reference naming a namespace rather than a token.
    #[error("line {line}: {path} references {target}, which is a namespace, not a token")]
    Namespace {
        /// The token holding the reference.
        path: TokenPath,
        /// The line the token is written on.
        line: usize,
        /// The path the reference names.
        target: TokenPath,
    },
    /// A set of tokens that reference each other in a loop.
    #[error("line {line}: reference cycle {}", cycle(.members))]
    Cycle {
        /// The tokens in the cycle, in the order the references run.
        members: Vec<TokenPath>,
        /// The line the first member is written on.
        line: usize,
    },
}

impl Problem {
    /// The line the problem sits on, or 0 when it is about the file as a whole.
    #[must_use]
    pub fn line(&self) -> usize {
        match self {
            Self::Key { line, .. }
            | Self::AnsiSlot { line, .. }
            | Self::Type { line, .. }
            | Self::Hex { line, .. }
            | Self::Reference { line, .. }
            | Self::MetaUnknown { line, .. }
            | Self::MetaVariant { line, .. }
            | Self::MetaFormat { line, .. }
            | Self::Undefined { line, .. }
            | Self::Namespace { line, .. }
            | Self::Cycle { line, .. } => *line,
            Self::MetaMissing { .. } => 0,
        }
    }
}

/// The cycle as `a -> b -> a`.
fn cycle(members: &[TokenPath]) -> String {
    members
        .iter()
        .chain(members.first())
        .map(TokenPath::as_str)
        .collect::<Vec<_>>()
        .join(" -> ")
}

/// The problems as one indented line each.
pub(super) fn listed(problems: &[Problem]) -> String {
    problems
        .iter()
        .map(|problem| format!("  {problem}"))
        .collect::<Vec<_>>()
        .join("\n")
}
