//! The golden test the renderer exists to pass.
//!
//! Every template in `tests/fixtures/templates/`, rendered against the reference theme, must
//! be byte-identical to its counterpart in `tests/fixtures/expected/`. A diff means the
//! renderer is wrong, not the fixture.

use std::fs;
use std::path::{Path, PathBuf};

use vanadis::{Template, Theme};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

fn fixtures() -> PathBuf {
    root().join("tests/fixtures")
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

/// The reference theme: `tests/fixtures/palette.json` in the format themes are written in.
///
/// `tests/theme.rs` is what proves the two carry the same colours.
fn palette() -> Theme {
    Theme::load(&root().join("docs/examples/papercolor-light.toml")).unwrap()
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
        let rendered = Template::new(&template, source)
            .render(palette.tokens())
            .unwrap();
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
