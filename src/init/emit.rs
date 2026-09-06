//! Writing out the three files: the theme, the target entry, and the merge into an existing
//! theme.
//!
//! `docs/init.md` decides what each holds. A theme `init` writes is flat — literal hex in
//! the namespace the answer named, with no `[colors]` layer to point at.

use std::collections::BTreeMap;
use std::fmt::Write as _;

use thiserror::Error;
use toml_edit::{DocumentMut, Item, Table, Value};

use crate::config::TargetName;
use crate::init::naming::display_name;
use crate::theme::{ThemeId, Variant};
use crate::token::TokenPath;

/// A file could not be written out.
#[derive(Debug, Clone, PartialEq, Eq, Error)]
pub enum EmitError {
    /// The theme already writes the key a token would be added under.
    #[error("the theme already writes `{path}`")]
    Occupied {
        /// The token that could not be added.
        path: TokenPath,
    },
    /// A token sits under something that is not a table.
    #[error("`{path}` cannot be written: `{at}` is not a table")]
    Blocked {
        /// The token that could not be added.
        path: TokenPath,
        /// The key that is in the way.
        at: String,
    },
}

/// A whole theme file holding `tokens`, in the format `docs/theme-format.md` decides.
#[must_use]
pub fn theme(id: &ThemeId, variant: Variant, tokens: &BTreeMap<TokenPath, String>) -> String {
    let mut file = String::new();
    let _ = writeln!(file, "[meta]");
    let _ = writeln!(file, "format = 1");
    let _ = writeln!(file, "name = {}", quoted(&display_name(id)));
    let _ = writeln!(file, "variant = {}", quoted(variant.as_str()));

    for (namespace, keys) in grouped(tokens) {
        let _ = writeln!(file);
        if !namespace.is_empty() {
            let _ = writeln!(file, "[{namespace}]");
        }
        for (key, value) in keys {
            let _ = writeln!(file, "{key} = {}", quoted(value));
        }
    }
    file
}

/// `source`, a theme file, with `tokens` added to it.
///
/// Everything already in the file keeps its formatting, its comments and its order, so a
/// second run adds lines to a theme somebody has been editing rather than rewriting it.
///
/// # Errors
///
/// Returns [`EmitError`] when a token cannot be added: the file already writes that key, or
/// something that is not a table sits on the path to it.
pub fn merged(source: &str, tokens: &BTreeMap<TokenPath, String>) -> Result<String, EmitError> {
    let Ok(mut document) = source.parse::<DocumentMut>() else {
        // The caller loaded this theme before offering to add to it.
        return Ok(source.to_owned());
    };

    for (path, value) in tokens {
        let segments: Vec<&str> = path.segments().collect();
        let (leaf, tables) = segments.split_last().unwrap_or((&"", &[]));
        let mut table = document.as_table_mut();
        for segment in tables {
            table = table
                .entry(segment)
                .or_insert_with(|| Item::Table(Table::new()))
                .as_table_mut()
                .ok_or_else(|| EmitError::Blocked {
                    path: path.clone(),
                    at: (*segment).to_owned(),
                })?;
        }
        table.set_implicit(false);
        if table.contains_key(leaf) {
            return Err(EmitError::Occupied { path: path.clone() });
        }
        table.insert(leaf, Item::Value(Value::from(value.clone())));
    }
    Ok(document.to_string())
}

/// `source`, a config file, with one more `[[targets]]` entry at the end of it.
///
/// The entry is appended as text. An array of tables goes last in a TOML file whatever else
/// the file holds, so nothing above it has to be re-encoded and nothing above it moves.
#[must_use]
pub fn target(source: &str, name: &TargetName, template: &str, output: &str) -> String {
    let mut config = source.to_owned();
    if !config.is_empty() {
        if !config.ends_with('\n') {
            config.push('\n');
        }
        config.push('\n');
    }
    let _ = writeln!(config, "[[targets]]");
    let _ = writeln!(config, "name = {}", quoted(name.as_str()));
    let _ = writeln!(config, "template = {}", quoted(template));
    let _ = writeln!(config, "output = {}", quoted(output));
    config
}

/// `tokens` gathered under the table each one sits in, tables in name order.
///
/// `meta` comes first so the file opens the way `docs/theme-format.md` writes it. A key that
/// is all digits sorts by its number, which is what keeps `[ansi]` reading 0 to 15 rather
/// than 0, 1, 10.
fn grouped(tokens: &BTreeMap<TokenPath, String>) -> Vec<(String, Vec<(&str, &str)>)> {
    let mut tables: BTreeMap<String, Vec<(&str, &str)>> = BTreeMap::new();
    for (path, value) in tokens {
        let segments: Vec<&str> = path.segments().collect();
        let Some((leaf, namespace)) = segments.split_last() else {
            continue;
        };
        tables
            .entry(namespace.join("."))
            .or_default()
            .push((leaf, value));
    }
    for keys in tables.values_mut() {
        keys.sort_by(
            |(left, _), (right, _)| match (left.parse::<u32>(), right.parse::<u32>()) {
                (Ok(left), Ok(right)) => left.cmp(&right),
                _ => left.cmp(right),
            },
        );
    }

    let mut grouped: Vec<(String, Vec<(&str, &str)>)> = tables.into_iter().collect();
    grouped.sort_by_key(|(namespace, _)| (namespace != "meta", namespace.clone()));
    grouped
}

/// `text` as a TOML string, quoted and escaped.
fn quoted(text: &str) -> String {
    Value::from(text).decorated("", "").to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn tokens(pairs: &[(&str, &str)]) -> BTreeMap<TokenPath, String> {
        pairs
            .iter()
            .map(|(path, value)| (TokenPath::parse(path).unwrap(), (*value).to_owned()))
            .collect()
    }

    fn written(pairs: &[(&str, &str)]) -> String {
        theme(
            &ThemeId::parse("papercolor-light").unwrap(),
            Variant::Light,
            &tokens(pairs),
        )
    }

    #[test]
    fn writes_the_metadata_a_theme_needs_to_load() {
        assert_eq!(
            written(&[]),
            "[meta]\nformat = 1\nname = \"Papercolor Light\"\nvariant = \"light\"\n"
        );
    }

    #[test]
    fn writes_a_token_under_its_own_table() {
        assert!(
            written(&[("role.bg", "#eeeeee")]).ends_with("\n[role]\nbg = \"#eeeeee\"\n"),
            "{}",
            written(&[("role.bg", "#eeeeee")])
        );
    }

    #[test]
    fn writes_each_table_once() {
        let file = written(&[("role.bg", "#eeeeee"), ("role.fg", "#444444")]);
        assert_eq!(file.matches("[role]").count(), 1);
    }

    #[test]
    fn writes_the_ansi_slots_in_numeric_order() {
        let file = written(&[("ansi.10", "#eeeeee"), ("ansi.2", "#444444")]);
        assert!(file.contains("2 = \"#444444\"\n10 = \"#eeeeee\""), "{file}");
    }

    #[test]
    fn writes_a_nested_namespace_as_one_table() {
        let file = written(&[("role.git.added", "#008700")]);
        assert!(file.contains("[role.git]\nadded = \"#008700\""), "{file}");
    }

    #[test]
    fn adds_a_token_to_a_theme_that_already_exists() {
        let source = "[meta]\nformat = 1\nname = \"Paper\"\nvariant = \"light\"\n\n[role]\nbg = \"#eeeeee\"\n";
        let merged = merged(source, &tokens(&[("role.fg", "#444444")])).unwrap();
        assert!(merged.contains("bg = \"#eeeeee\""), "{merged}");
        assert!(merged.contains("fg = \"#444444\""), "{merged}");
    }

    #[test]
    fn keeps_a_comment_the_theme_already_had() {
        let source = "# hand written\n[meta]\nformat = 1\nname = \"Paper\"\nvariant = \"light\"\n";
        let merged = merged(source, &tokens(&[("role.fg", "#444444")])).unwrap();
        assert!(merged.starts_with("# hand written\n"), "{merged}");
    }

    #[test]
    fn opens_a_table_the_theme_does_not_have_yet() {
        let source = "[meta]\nformat = 1\nname = \"Paper\"\nvariant = \"light\"\n";
        let merged = merged(source, &tokens(&[("colors.paper", "#eeeeee")])).unwrap();
        assert!(merged.contains("[colors]"), "{merged}");
    }

    #[test]
    fn refuses_to_overwrite_a_key_the_theme_already_writes() {
        let source =
            "[meta]\nformat = 1\nname = \"Paper\"\nvariant = \"light\"\n[role]\nbg = \"#eeeeee\"\n";
        assert_eq!(
            merged(source, &tokens(&[("role.bg", "#444444")])),
            Err(EmitError::Occupied {
                path: TokenPath::parse("role.bg").unwrap()
            })
        );
    }

    #[test]
    fn reports_a_key_that_is_not_a_table_on_the_way_to_a_token() {
        let source = "role = 1\n[meta]\nformat = 1\nname = \"Paper\"\nvariant = \"light\"\n";
        assert!(matches!(
            merged(source, &tokens(&[("role.bg", "#eeeeee")])),
            Err(EmitError::Blocked { .. })
        ));
    }

    #[test]
    fn appends_a_target_to_a_config_that_has_one() {
        let source = "[[targets]]\nname = \"btop\"\ntemplate = \"a.in\"\noutput = \"a\"\n";
        let config = target(
            source,
            &TargetName::parse("nvim").unwrap(),
            "templates/nvim/palette.lua.in",
            "~/.config/nvim/lua/palette.lua",
        );
        assert_eq!(
            config,
            format!(
                "{source}\n[[targets]]\nname = \"nvim\"\ntemplate = \"templates/nvim/palette.lua.in\"\noutput = \"~/.config/nvim/lua/palette.lua\"\n"
            )
        );
    }

    #[test]
    fn writes_the_first_target_of_a_config_that_did_not_exist() {
        let config = target(
            "",
            &TargetName::parse("nvim").unwrap(),
            "templates/nvim/palette.lua.in",
            "~/.config/nvim/lua/palette.lua",
        );
        assert!(config.starts_with("[[targets]]\n"), "{config}");
    }

    #[test]
    fn keeps_the_auto_table_a_config_already_had() {
        let source = "[auto]\nlight = \"paper\"\ndark = \"ink\"\n";
        let config = target(source, &TargetName::parse("nvim").unwrap(), "a.in", "a");
        assert!(config.starts_with(source), "{config}");
    }

    #[test]
    fn quotes_a_path_with_a_quote_in_it_so_it_reads_back() {
        let config = target("", &TargetName::parse("nvim").unwrap(), "a.in", "a\"b");
        let document = config.parse::<DocumentMut>().unwrap();
        assert_eq!(
            document["targets"][0]["output"].as_str(),
            Some("a\"b"),
            "{config}"
        );
    }
}
