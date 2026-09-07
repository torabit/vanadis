//! The theme loader against the reference theme.
//!
//! `docs/examples/papercolor-light.toml` is `tests/fixtures/palette.json` written in the
//! format `docs/theme-format.md` decides. Loading it must produce the same values the JSON
//! carries, or the format does not round-trip.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use coloris::{Theme, TokenPath};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

/// `tests/fixtures/palette.json`, with its `{path}` references followed.
fn json_palette() -> BTreeMap<String, String> {
    let text = fs::read_to_string(root().join("tests/fixtures/palette.json")).unwrap();
    let namespaces: BTreeMap<String, BTreeMap<String, String>> =
        serde_json::from_str(&text).unwrap();

    let raw: BTreeMap<String, String> = namespaces
        .iter()
        .flat_map(|(namespace, entries)| {
            entries
                .iter()
                .map(move |(key, value)| (format!("{namespace}.{key}"), value.clone()))
        })
        .collect();

    raw.iter()
        .map(|(path, value)| {
            let mut current = value.clone();
            while let Some(target) = current
                .strip_prefix('{')
                .and_then(|rest| rest.strip_suffix('}'))
            {
                current = raw[target].clone();
            }
            (path.clone(), current)
        })
        .collect()
}

fn example() -> Theme {
    Theme::load(&root().join("docs/examples/papercolor-light.toml")).unwrap()
}

#[test]
fn loads_the_example_theme() {
    assert_eq!(example().id().as_str(), "papercolor-light");
}

#[test]
fn resolves_every_colour_the_json_palette_carries() {
    let theme = example();
    let palette = json_palette();
    let missing: Vec<&String> = palette
        .iter()
        .filter(|(path, value)| {
            TokenPath::parse(path)
                .and_then(|path| theme.tokens().get(&path).map(str::to_owned))
                .as_ref()
                != Some(*value)
        })
        .map(|(path, _)| path)
        .collect();

    assert!(
        missing.is_empty(),
        "not resolved as the JSON has them: {missing:?}"
    );
}
