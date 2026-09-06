//! Token paths and the flat table a theme resolves to.

use std::collections::BTreeMap;
use std::fmt;

/// A token path: one or more segments joined by `.`.
#[derive(Debug, Clone, PartialEq, Eq, PartialOrd, Ord, Hash)]
pub struct TokenPath(String);

impl TokenPath {
    /// Parses `text` as a token path, returning `None` when it is not one.
    ///
    /// Segments are lowercase ASCII, digits and internal hyphens, joined by `.`.
    #[must_use]
    pub fn parse(text: &str) -> Option<Self> {
        text.split('.')
            .all(is_segment)
            .then(|| Self(text.to_owned()))
    }

    /// The path as written.
    #[must_use]
    pub fn as_str(&self) -> &str {
        &self.0
    }

    /// The first segment, which names the namespace a token sits in.
    #[must_use]
    pub fn root(&self) -> &str {
        self.0.split('.').next().unwrap_or_default()
    }

    /// The segments the path is made of.
    pub fn segments(&self) -> impl Iterator<Item = &str> {
        self.0.split('.')
    }

    /// `self.segment`, or `None` when `segment` is not a valid segment.
    #[must_use]
    pub fn child(&self, segment: &str) -> Option<Self> {
        is_segment(segment).then(|| Self(format!("{}.{segment}", self.0)))
    }

    /// Joins pre-validated segments. The caller guarantees each one is a segment.
    pub(crate) fn from_segments<'a>(segments: impl IntoIterator<Item = &'a str>) -> Self {
        Self(segments.into_iter().collect::<Vec<_>>().join("."))
    }
}

/// One segment of a token path: `[a-z0-9]([a-z0-9-]*[a-z0-9])?`.
pub(crate) fn is_segment(text: &str) -> bool {
    let bytes = text.as_bytes();
    let (Some(&first), Some(&last)) = (bytes.first(), bytes.last()) else {
        return false;
    };
    let edge = |byte: u8| byte.is_ascii_lowercase() || byte.is_ascii_digit();
    edge(first) && edge(last) && bytes.iter().all(|&byte| edge(byte) || byte == b'-')
}

impl fmt::Display for TokenPath {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(&self.0)
    }
}

/// Every token path a theme defines, mapped to its resolved value.
#[derive(Debug, Clone, Default, PartialEq, Eq)]
pub struct Tokens(BTreeMap<TokenPath, String>);

impl Tokens {
    /// The value bound to `path`, or `None` when the theme does not define it.
    #[must_use]
    pub fn get(&self, path: &TokenPath) -> Option<&str> {
        self.0.get(path).map(String::as_str)
    }
}

impl FromIterator<(TokenPath, String)> for Tokens {
    fn from_iter<I: IntoIterator<Item = (TokenPath, String)>>(iter: I) -> Self {
        Self(iter.into_iter().collect())
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn parses(text: &str) -> bool {
        TokenPath::parse(text).is_some()
    }

    #[test]
    fn parses_a_two_segment_path() {
        assert_eq!(TokenPath::parse("role.bg").unwrap().as_str(), "role.bg");
    }

    #[test]
    fn parses_a_single_segment() {
        assert!(parses("bg"));
    }

    #[test]
    fn parses_digits_as_a_segment() {
        assert!(parses("ansi.15"));
    }

    #[test]
    fn parses_an_internal_hyphen() {
        assert!(parses("role.accent-alt"));
    }

    #[test]
    fn rejects_the_empty_string() {
        assert!(!parses(""));
    }

    #[test]
    fn rejects_surrounding_space() {
        assert!(!parses(" title "));
    }

    #[test]
    fn rejects_a_wildcard() {
        assert!(!parses("role.*"));
    }

    #[test]
    fn rejects_uppercase() {
        assert!(!parses("Role.bg"));
    }

    #[test]
    fn rejects_an_underscore() {
        assert!(!parses("role.added_bg"));
    }

    #[test]
    fn rejects_an_empty_segment() {
        assert!(!parses("role."));
    }

    #[test]
    fn rejects_a_leading_hyphen() {
        assert!(!parses("role.-bg"));
    }

    #[test]
    fn rejects_a_trailing_hyphen() {
        assert!(!parses("role.bg-"));
    }

    #[test]
    fn looks_up_a_defined_token() {
        let tokens: Tokens = [(TokenPath::parse("role.bg").unwrap(), "#eeeeee".to_owned())]
            .into_iter()
            .collect();
        assert_eq!(
            tokens.get(&TokenPath::parse("role.bg").unwrap()),
            Some("#eeeeee")
        );
    }

    #[test]
    fn returns_none_for_an_undefined_token() {
        let tokens = Tokens::default();
        assert_eq!(tokens.get(&TokenPath::parse("role.bg").unwrap()), None);
    }
}
