//! Reading the document into definitions, and everything checkable while doing so.

use std::collections::BTreeMap;

use toml_edit::{Item, Table, Value};

use super::Problem;
use crate::token::TokenPath;

/// Namespaces whose values are strings that are never parsed as colours.
const OPAQUE: [&str; 2] = ["meta", "text"];

/// One token as the file writes it, before references are followed.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) struct Definition {
    pub(super) value: RawValue,
    pub(super) line: usize,
}

/// A theme value: a literal, or exactly one reference filling the whole string.
#[derive(Debug, Clone, PartialEq, Eq)]
pub(super) enum RawValue {
    Literal(String),
    Reference(TokenPath),
}

/// What reading the document found.
pub(super) struct Walked {
    /// Every token the file defines, still holding references.
    pub(super) definitions: BTreeMap<TokenPath, Definition>,
    /// Every namespace the file opens, and the line it opens on.
    pub(super) namespaces: BTreeMap<TokenPath, usize>,
    /// Every problem visible without following a reference.
    pub(super) problems: Vec<Problem>,
}

/// Reads `document`, which is `source` parsed.
pub(super) fn walk(source: &str, document: &Table) -> Walked {
    let mut walk = Walk {
        source,
        walked: Walked {
            definitions: BTreeMap::new(),
            namespaces: BTreeMap::new(),
            problems: Vec::new(),
        },
    };
    walk.table(document, None);
    walk.walked
}

/// Collects every definition and namespace in the document, and every problem with them.
struct Walk<'a> {
    source: &'a str,
    walked: Walked,
}

impl Walk<'_> {
    /// Walks `table`, whose own path is `prefix`.
    fn table(&mut self, table: &Table, prefix: Option<&TokenPath>) {
        for (key, item) in table {
            let line = table
                .key(key)
                .and_then(toml_edit::Key::span)
                .map_or(0, |span| self.line_of(span.start));

            let Some(path) = child(prefix, key) else {
                self.walked.problems.push(Problem::Key {
                    key: key.to_owned(),
                    line,
                });
                continue;
            };

            if prefix.is_some_and(|prefix| prefix.as_str() == "ansi")
                && (!is_slot(key) || item.is_table_like())
            {
                self.walked.problems.push(Problem::AnsiSlot {
                    key: key.to_owned(),
                    line,
                });
                continue;
            }

            self.entry(&path, item, line);
        }
    }

    /// Records one key, which is either a namespace or a token.
    fn entry(&mut self, path: &TokenPath, item: &Item, line: usize) {
        match item {
            Item::Table(table) => {
                self.walked.namespaces.insert(path.clone(), line);
                self.table(table, Some(path));
            }
            Item::Value(Value::InlineTable(inline)) => {
                self.walked.namespaces.insert(path.clone(), line);
                for (key, value) in inline {
                    let Some(nested) = child(Some(path), key) else {
                        self.walked.problems.push(Problem::Key {
                            key: key.to_owned(),
                            line,
                        });
                        continue;
                    };
                    self.entry(&nested, &Item::Value(value.clone()), line);
                }
            }
            _ if is_meta_format(path) => self.format(path, item, line),
            Item::Value(Value::String(text)) => {
                let value = self.value(path, text.value(), line);
                self.walked
                    .definitions
                    .insert(path.clone(), Definition { value, line });
            }
            other => self.walked.problems.push(Problem::Type {
                path: path.clone(),
                line,
                found: other.type_name(),
            }),
        }
    }

    /// Records `meta.format`, which is the one value written as a number.
    ///
    /// A version it cannot read is still recorded, so that a file carrying one is a theme
    /// with a bad `format` rather than a theme missing `format`.
    fn format(&mut self, path: &TokenPath, item: &Item, line: usize) {
        let version = item.as_integer();
        let found = version.map_or_else(
            || item.type_name().to_owned(),
            |version| version.to_string(),
        );

        if version != Some(1) {
            self.walked.problems.push(Problem::MetaFormat {
                line,
                found: found.clone(),
            });
        }

        self.walked.definitions.insert(
            path.clone(),
            Definition {
                value: RawValue::Literal(found),
                line,
            },
        );
    }

    /// Classifies a string value by the namespace it sits in.
    fn value(&mut self, path: &TokenPath, text: &str, line: usize) -> RawValue {
        if is_opaque(path) {
            return RawValue::Literal(text.to_owned());
        }
        if let Some(target) = reference(text) {
            return RawValue::Reference(target);
        }
        if is_hex(text) {
            return RawValue::Literal(text.to_ascii_lowercase());
        }

        self.walked.problems.push(if text.contains("{{") {
            Problem::Reference {
                path: path.clone(),
                line,
                value: text.to_owned(),
            }
        } else {
            Problem::Hex {
                path: path.clone(),
                line,
                value: text.to_owned(),
            }
        });
        RawValue::Literal(text.to_owned())
    }

    /// The 1-based line `offset` falls on.
    fn line_of(&self, offset: usize) -> usize {
        self.source[..offset]
            .bytes()
            .filter(|&b| b == b'\n')
            .count()
            + 1
    }
}

/// Every problem with `[meta]`, which is the one table the format fixes the keys of.
pub(super) fn meta_problems(
    definitions: &BTreeMap<TokenPath, Definition>,
    namespaces: &BTreeMap<TokenPath, usize>,
) -> Vec<Problem> {
    const KEYS: [&str; 4] = ["format", "name", "variant", "author"];

    let mut problems = Vec::new();
    let written = definitions
        .iter()
        .map(|(path, definition)| (path, definition.line))
        .chain(namespaces.iter().map(|(path, line)| (path, *line)));

    for (path, line) in written {
        let mut segments = path.segments();
        if segments.next() != Some("meta") {
            continue;
        }
        match segments.next() {
            Some(key) if !KEYS.contains(&key) && segments.next().is_none() => {
                problems.push(Problem::MetaUnknown {
                    key: key.to_owned(),
                    line,
                });
            }
            _ => {}
        }
    }

    for key in ["format", "name", "variant"] {
        if !definitions.contains_key(&meta_path(key)) {
            problems.push(Problem::MetaMissing {
                key: key.to_owned(),
            });
        }
    }

    if let Some(variant) = definitions.get(&meta_path("variant"))
        && let RawValue::Literal(value) = &variant.value
        && value != "dark"
        && value != "light"
    {
        problems.push(Problem::MetaVariant {
            line: variant.line,
            value: value.clone(),
        });
    }

    problems
}

/// Whether `key` names one of the sixteen ANSI slots: `0` through `15`, no leading zeros.
fn is_slot(key: &str) -> bool {
    key.parse::<u8>()
        .is_ok_and(|slot| slot <= 15 && key == slot.to_string())
}

/// Whether `text` is a hex literal, which [`super::rgb`] decides.
fn is_hex(text: &str) -> bool {
    super::rgb(text).is_some()
}

/// `prefix.key`, or `None` when `key` is not a valid segment.
fn child(prefix: Option<&TokenPath>, key: &str) -> Option<TokenPath> {
    match prefix {
        Some(prefix) => prefix.child(key),
        None => TokenPath::parse(key),
    }
}

/// The token path a whole-string `{{reference}}` names.
fn reference(text: &str) -> Option<TokenPath> {
    let inner = text.strip_prefix("{{")?.strip_suffix("}}")?;
    TokenPath::parse(inner)
}

/// `meta.key`.
pub(super) fn meta_path(key: &str) -> TokenPath {
    TokenPath::from_segments(["meta", key])
}

/// Whether `path` sits in a namespace whose values are never colours.
fn is_opaque(path: &TokenPath) -> bool {
    OPAQUE.contains(&path.root())
}

/// Whether `path` is `meta.format`.
fn is_meta_format(path: &TokenPath) -> bool {
    *path == meta_path("format")
}
