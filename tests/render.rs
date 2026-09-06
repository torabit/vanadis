//! The golden test the renderer exists to pass.
//!
//! Every template in `tests/fixtures/templates/`, rendered against
//! `tests/fixtures/palette.json`, must be byte-identical to its counterpart in
//! `tests/fixtures/expected/`. A diff means the renderer is wrong, not the fixture.

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use vanadis::{Template, TokenPath, Tokens};

fn fixtures() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).join("tests/fixtures")
}

/// The fixture templates, as `MANIFEST.tsv` lists them.
fn manifest() -> Vec<PathBuf> {
    let text = fs::read_to_string(fixtures().join("MANIFEST.tsv")).unwrap();
    text.lines()
        .skip(1)
        .filter(|line| !line.is_empty())
        .map(|line| fixtures().join(line.split('\t').next().unwrap()))
        .collect()
}

/// `tests/fixtures/palette.json` as a flat token table.
///
/// The JSON fixture predates the theme format and writes a reference as `{colors.paper}`.
/// Resolving it here is the theme loader's job, which this test does not have yet.
fn palette() -> Tokens {
    let text = fs::read_to_string(fixtures().join("palette.json")).unwrap();
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
            let resolved = resolve(value, &raw);
            (TokenPath::parse(path).unwrap(), resolved)
        })
        .collect()
}

/// Follows a chain of `{path}` references down to a literal.
fn resolve(value: &str, raw: &BTreeMap<String, String>) -> String {
    let mut current = value.to_owned();
    for _ in 0..raw.len() {
        let Some(target) = current
            .strip_prefix('{')
            .and_then(|rest| rest.strip_suffix('}'))
        else {
            return current;
        };
        current = raw
            .get(target)
            .unwrap_or_else(|| panic!("undefined reference {target}"))
            .clone();
    }
    panic!("reference cycle reaching {value}")
}

#[test]
fn renders_every_fixture_template_byte_for_byte() {
    let palette = palette();
    let templates = manifest();
    assert_eq!(
        templates.len(),
        11,
        "MANIFEST.tsv should list eleven templates"
    );

    for template in templates {
        let relative = template
            .strip_prefix(fixtures().join("templates"))
            .unwrap()
            .to_owned();
        let expected_path = fixtures()
            .join("expected")
            .join(relative.with_extension(""));

        let source = fs::read_to_string(&template).unwrap();
        let rendered = Template::new(&template, source).render(&palette).unwrap();
        let expected = fs::read_to_string(&expected_path).unwrap();

        assert_eq!(
            rendered,
            expected,
            "{} does not match {}",
            template.display(),
            expected_path.display()
        );
    }
}
