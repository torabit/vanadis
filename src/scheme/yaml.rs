//! Reading the YAML an upstream scheme is written in.
//!
//! Upstream schemes are a flat mapping of metadata plus one nested mapping of colours,
//! whatever the system. Everything here is about that shape and nothing here knows a slot
//! name, so a converter for another system reads its document through the same four
//! functions.
//!
//! This is the only layer that touches the YAML. [`Scheme::parse`](super::Scheme::parse)
//! reads a header through it and the converter reads a palette through it, so an absent key
//! and a key of the wrong type have one answer and not one per caller.
//!
//! Keys the caller does not read are ignored. `system` is dropped by `docs/schemes.md`,
//! which takes the system from the directory instead, and upstream also writes `slug` and
//! `description`, none of which `[meta]` has a home for.

use saphyr::{LoadableYamlNode, Yaml};

use super::Problem;

/// The scheme's top-level mapping.
///
/// A stream carrying more than one document is read as its first, which is what a scheme
/// file with a trailing `---` amounts to.
///
/// # Errors
///
/// Returns [`Problem::Syntax`] when the text is not YAML, [`Problem::Empty`] when it holds
/// no document, and [`Problem::Document`] when the document is not a mapping.
pub(super) fn document(text: &str) -> Result<Yaml<'_>, Problem> {
    let documents =
        Yaml::load_from_str(text).map_err(|error| Problem::Syntax(error.to_string()))?;
    let document = documents.into_iter().next().ok_or(Problem::Empty)?;
    if document.is_mapping() {
        Ok(document)
    } else {
        Err(Problem::Document)
    }
}

/// The string `key` holds, or `None` when the mapping does not write it.
///
/// # Errors
///
/// Returns [`Problem::Type`] when `key` is written and does not hold a string.
pub(super) fn optional<'a>(mapping: &'a Yaml<'_>, key: &str) -> Result<Option<&'a str>, Problem> {
    match mapping.as_mapping_get(key) {
        None => Ok(None),
        Some(value) => value.as_str().map(Some).ok_or_else(|| Problem::Type {
            key: key.to_owned(),
            expected: "a string",
        }),
    }
}

/// The string `key` holds.
///
/// # Errors
///
/// Returns [`Problem::Missing`] when `key` is not written and [`Problem::Type`] when it does
/// not hold a string.
pub(super) fn required<'a>(mapping: &'a Yaml<'_>, key: &'static str) -> Result<&'a str, Problem> {
    optional(mapping, key)?.ok_or(Problem::Missing(key))
}

/// The mapping `key` holds.
///
/// # Errors
///
/// Returns [`Problem::Missing`] when `key` is not written and [`Problem::Type`] when it does
/// not hold a mapping.
pub(super) fn nested<'a, 'input>(
    mapping: &'a Yaml<'input>,
    key: &'static str,
) -> Result<&'a Yaml<'input>, Problem> {
    let value = mapping.as_mapping_get(key).ok_or(Problem::Missing(key))?;
    if value.is_mapping() {
        Ok(value)
    } else {
        Err(Problem::Type {
            key: key.to_owned(),
            expected: "a mapping",
        })
    }
}

/// Whether the mapping writes `key` at all, whatever kind of value it holds.
///
/// Separate from reading the key, because a palette answers "does the scheme carry this
/// colour" before anything reads what the colour is, and a type error is not a no.
pub(super) fn writes(mapping: &Yaml<'_>, key: &str) -> bool {
    mapping.as_mapping_get(key).is_some()
}

/// The entries of `mapping`, in the order the document writes them.
///
/// `at` names the mapping, for the error a caller cannot otherwise attribute.
///
/// # Errors
///
/// Returns [`Problem::Type`] when `mapping` is not a mapping, or writes a key that is not a
/// string.
pub(super) fn entries<'a, 'input>(
    mapping: &'a Yaml<'input>,
    at: &str,
) -> Result<Vec<(&'a str, &'a Yaml<'input>)>, Problem> {
    mapping
        .as_mapping()
        .ok_or_else(|| Problem::Type {
            key: at.to_owned(),
            expected: "a mapping",
        })?
        .iter()
        .map(|(key, value)| {
            key.as_str()
                .map(|key| (key, value))
                .ok_or_else(|| Problem::Type {
                    key: at.to_owned(),
                    expected: "keyed by strings",
                })
        })
        .collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn reads_a_string_off_the_document() {
        let document = document("system: \"base16\"\n").unwrap();
        assert_eq!(required(&document, "system").unwrap(), "base16");
    }

    #[test]
    fn returns_none_for_a_key_the_scheme_does_not_write() {
        let document = document("system: \"base16\"\n").unwrap();
        assert_eq!(optional(&document, "variant").unwrap(), None);
    }

    #[test]
    fn reports_a_key_the_scheme_has_to_write() {
        let document = document("system: \"base16\"\n").unwrap();
        assert_eq!(
            required(&document, "name").unwrap_err(),
            Problem::Missing("name")
        );
    }

    #[test]
    fn reports_a_key_that_does_not_hold_a_string() {
        let document = document("name:\n  first: \"Gruvbox\"\n").unwrap();
        assert!(matches!(
            required(&document, "name"),
            Err(Problem::Type { ref key, .. }) if key == "name"
        ));
    }

    #[test]
    fn reads_a_nested_mapping() {
        let document = document("palette:\n  base00: \"#1d2021\"\n").unwrap();
        let palette = nested(&document, "palette").unwrap();
        assert_eq!(required(palette, "base00").unwrap(), "#1d2021");
    }

    #[test]
    fn reports_a_key_that_does_not_hold_a_mapping() {
        let document = document("palette: \"#1d2021\"\n").unwrap();
        assert!(matches!(
            nested(&document, "palette"),
            Err(Problem::Type { ref key, .. }) if key == "palette"
        ));
    }

    #[test]
    fn reports_yaml_that_does_not_parse() {
        assert!(matches!(
            document("name: \"unterminated\n"),
            Err(Problem::Syntax(_))
        ));
    }

    #[test]
    fn reports_a_stream_holding_no_document() {
        assert_eq!(document("").unwrap_err(), Problem::Empty);
    }

    #[test]
    fn reports_a_document_that_is_not_a_mapping() {
        assert_eq!(document("- base00\n").unwrap_err(), Problem::Document);
    }

    #[test]
    fn says_a_key_the_scheme_writes_is_written() {
        let document = document("scheme:\n  system: \"tinted8\"\n").unwrap();
        assert!(writes(&document, "scheme"));
    }

    #[test]
    fn says_a_key_the_scheme_omits_is_not_written() {
        let document = document("system: \"base16\"\n").unwrap();
        assert!(!writes(&document, "scheme"));
    }

    #[test]
    fn reads_the_entries_of_a_mapping_in_the_order_they_are_written() {
        let document = document("ui:\n  gutter: \"#1d2021\"\n  accent: \"#fabd2f\"\n").unwrap();
        let ui = nested(&document, "ui").unwrap();
        let keys: Vec<&str> = entries(ui, "ui")
            .unwrap()
            .into_iter()
            .map(|(key, _)| key)
            .collect();
        assert_eq!(keys, ["gutter", "accent"]);
    }

    /// The parser folds a key the scheme writes twice, keeping the value written last, so
    /// no caller sees one key twice and none has to guard against it.
    #[test]
    fn folds_a_key_the_scheme_writes_twice() {
        let document = document("ui:\n  red: \"#111111\"\n  red: \"#222222\"\n").unwrap();
        let ui = nested(&document, "ui").unwrap();
        let entries = entries(ui, "ui").unwrap();
        assert_eq!(entries.len(), 1);
        assert_eq!(entries[0].0, "red");
        assert_eq!(entries[0].1.as_str(), Some("#222222"));
    }

    #[test]
    fn reports_a_mapping_that_is_not_keyed_by_strings() {
        let document = document("ui:\n  1: \"#1d2021\"\n").unwrap();
        let ui = nested(&document, "ui").unwrap();
        assert!(matches!(
            entries(ui, "ui"),
            Err(Problem::Type { ref key, .. }) if key == "ui"
        ));
    }
}
