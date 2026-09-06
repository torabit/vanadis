//! Reading the YAML an upstream scheme is written in.
//!
//! Upstream schemes are a flat mapping of metadata plus one nested mapping of colours,
//! whatever the system. Everything here is about that shape and nothing here knows a slot
//! name, so a converter for another system reads its document through the same four
//! functions.
//!
//! Keys the converter does not read are ignored. `system` is dropped by
//! `docs/theme-format.md` and upstream also writes `slug` and `description`, none of which
//! `[meta]` has a home for.

use saphyr::{LoadableYamlNode, Yaml};

use super::SchemeError;

/// The scheme's top-level mapping.
///
/// A stream carrying more than one document is read as its first, which is what a scheme
/// file with a trailing `---` amounts to.
///
/// # Errors
///
/// Returns [`SchemeError::Yaml`] when the text is not YAML and [`SchemeError::Document`]
/// when it holds no document or one that is not a mapping.
pub(super) fn document(text: &str) -> Result<Yaml<'_>, SchemeError> {
    let documents = Yaml::load_from_str(text).map_err(|source| SchemeError::Yaml { source })?;
    let document = documents.into_iter().next().ok_or(SchemeError::Document)?;
    if document.is_mapping() {
        Ok(document)
    } else {
        Err(SchemeError::Document)
    }
}

/// The string `key` holds, or `None` when the mapping does not write it.
///
/// # Errors
///
/// Returns [`SchemeError::Type`] when `key` is written and does not hold a string.
pub(super) fn optional<'a>(
    mapping: &'a Yaml<'_>,
    key: &str,
) -> Result<Option<&'a str>, SchemeError> {
    match mapping.as_mapping_get(key) {
        None => Ok(None),
        Some(value) => value.as_str().map(Some).ok_or_else(|| SchemeError::Type {
            key: key.to_owned(),
            expected: "a string",
        }),
    }
}

/// The string `key` holds.
///
/// # Errors
///
/// Returns [`SchemeError::Missing`] when `key` is not written and [`SchemeError::Type`]
/// when it does not hold a string.
pub(super) fn required<'a>(mapping: &'a Yaml<'_>, key: &str) -> Result<&'a str, SchemeError> {
    optional(mapping, key)?.ok_or_else(|| SchemeError::Missing {
        key: key.to_owned(),
    })
}

/// The mapping `key` holds.
///
/// # Errors
///
/// Returns [`SchemeError::Missing`] when `key` is not written and [`SchemeError::Type`]
/// when it does not hold a mapping.
pub(super) fn nested<'a, 'input>(
    mapping: &'a Yaml<'input>,
    key: &str,
) -> Result<&'a Yaml<'input>, SchemeError> {
    let value = mapping
        .as_mapping_get(key)
        .ok_or_else(|| SchemeError::Missing {
            key: key.to_owned(),
        })?;
    if value.is_mapping() {
        Ok(value)
    } else {
        Err(SchemeError::Type {
            key: key.to_owned(),
            expected: "a mapping",
        })
    }
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
    fn reads_an_unquoted_string() {
        let document = document("system: base16\n").unwrap();
        assert_eq!(required(&document, "system").unwrap(), "base16");
    }

    #[test]
    fn ignores_a_key_nothing_reads() {
        let document = document("system: \"base16\"\nslug: \"gruvbox\"\n").unwrap();
        assert_eq!(required(&document, "system").unwrap(), "base16");
    }

    #[test]
    fn reads_a_comment_off_the_end_of_a_value() {
        let document = document("name: \"Gruvbox\" # upstream\n").unwrap();
        assert_eq!(required(&document, "name").unwrap(), "Gruvbox");
    }

    #[test]
    fn returns_none_for_a_key_the_scheme_does_not_write() {
        let document = document("system: \"base16\"\n").unwrap();
        assert_eq!(optional(&document, "variant").unwrap(), None);
    }

    #[test]
    fn reports_a_key_the_scheme_has_to_write() {
        let document = document("system: \"base16\"\n").unwrap();
        assert!(matches!(
            required(&document, "name"),
            Err(SchemeError::Missing { ref key }) if key == "name"
        ));
    }

    #[test]
    fn reports_a_key_that_does_not_hold_a_string() {
        let document = document("name:\n  first: \"Gruvbox\"\n").unwrap();
        assert!(matches!(
            required(&document, "name"),
            Err(SchemeError::Type { ref key, .. }) if key == "name"
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
            Err(SchemeError::Type { ref key, .. }) if key == "palette"
        ));
    }

    #[test]
    fn reports_yaml_that_does_not_parse() {
        assert!(matches!(
            document("name: \"unterminated\n"),
            Err(SchemeError::Yaml { .. })
        ));
    }

    #[test]
    fn reports_an_empty_stream() {
        assert!(matches!(document(""), Err(SchemeError::Document)));
    }

    #[test]
    fn reports_a_document_that_is_not_a_mapping() {
        assert!(matches!(document("- base00\n"), Err(SchemeError::Document)));
    }
}
