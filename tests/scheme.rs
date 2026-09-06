//! The two things converting an upstream scheme has to achieve.
//!
//! `docs/core-vocabulary.md` requires a converter to fill the entire core, so a converted
//! scheme has to emit, load back, and answer `vocabulary::missing` with nothing. It also has
//! to render the templates the core exists for.
//!
//! The fixtures under `tests/fixtures/schemes/` are upstream files, copied verbatim from the
//! `spec-0.11` branch of `tinted-theming/schemes`, which is that repository's default
//! branch. They are not listed in `MANIFEST.tsv`, which maps a fixture to the file in the
//! dotfiles it came from; these came from a scheme repository instead.

use std::fs;
use std::path::{Path, PathBuf};

use vanadis::{Template, Theme, ThemeId, TokenPath, Variant, convert, vocabulary};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

fn fixtures() -> PathBuf {
    root().join("tests/fixtures")
}

/// The converted scheme, written out as a theme file and loaded back.
fn theme(scheme: &str, id: &str) -> Theme {
    let bytes = fs::read(fixtures().join("schemes").join(scheme)).unwrap();
    let converted = convert(&bytes).unwrap();
    let id = ThemeId::parse(id).unwrap();
    let file =
        vanadis::init::theme(converted.name(), converted.variant(), converted.tokens()).unwrap();
    Theme::parse(&root().join("themes").join(format!("{id}.toml")), &file).unwrap()
}

/// Every fixture scheme, with the identifier its filename gives the theme it becomes.
fn schemes() -> Vec<(&'static str, &'static str)> {
    vec![
        ("base16/gruvbox-dark-hard.yaml", "gruvbox-dark-hard"),
        ("base16/solarized-light.yaml", "solarized-light"),
        ("base24/dracula.yaml", "dracula"),
        ("base24/papercolor-light.yaml", "papercolor-light"),
    ]
}

#[test]
fn fills_the_whole_core_from_every_upstream_scheme() {
    for (scheme, id) in schemes() {
        let theme = theme(scheme, id);
        assert_eq!(
            vocabulary::missing(theme.tokens()),
            Vec::new(),
            "{scheme} leaves core tokens undefined"
        );
    }
}

/// Every core role against the colour `base16/gruvbox-dark-hard.yaml` writes.
///
/// The expected column is written out rather than read back off the converter's own table,
/// so a wrong row there fails here instead of agreeing with itself. Each pair is taken from
/// `docs/core-vocabulary.md`, "base16 onto the core".
#[test]
fn resolves_every_core_role_to_the_colour_the_mapping_names() {
    let theme = theme("base16/gruvbox-dark-hard.yaml", "gruvbox-dark-hard");
    let expected = [
        ("bg", "#1d2021"),           // base00
        ("fg", "#d5c4a1"),           // base05
        ("comment", "#665c54"),      // base03
        ("keyword", "#d3869b"),      // base0E
        ("string", "#b8bb26"),       // base0B
        ("error", "#fb4934"),        // base08
        ("ok", "#b8bb26"),           // base0B
        ("warn", "#fabd2f"),         // base0A
        ("visual", "#83a598"),       // base0D
        ("linenr", "#bdae93"),       // base04
        ("accent", "#83a598"),       // base0D
        ("accent-alt", "#8ec07c"),   // base0C
        ("accent-warm", "#fe8019"),  // base09
        ("inactive", "#665c54"),     // base03
        ("border", "#504945"),       // base02
        ("selection-bg", "#504945"), // base02
        ("hover-bg", "#3c3836"),     // base01
    ];
    assert_eq!(expected.len(), 17, "the core names seventeen roles");

    for (role, colour) in expected {
        let path = TokenPath::parse(&format!("role.{role}")).unwrap();
        assert_eq!(theme.tokens().get(&path), Some(colour), "role.{role}");
    }
}

#[test]
fn keeps_the_upstream_name_and_variant() {
    let theme = theme("base16/gruvbox-dark-hard.yaml", "gruvbox-dark-hard");
    assert_eq!(theme.name(), "Gruvbox dark, hard");
    assert_eq!(theme.variant(), Variant::Dark);
}

#[test]
fn renders_a_core_only_template_against_a_converted_scheme() {
    let theme = theme("base16/gruvbox-dark-hard.yaml", "gruvbox-dark-hard");
    let path = fixtures().join("templates/zsh/palette.zsh.in");
    let source = fs::read_to_string(&path).unwrap();

    let rendered = Template::new(&path, source).render(theme.tokens()).unwrap();

    // `--color=light` is the template's own hard-coded line, which
    // `docs/theme-format.md` records as the reason `meta.variant` is readable as a token.
    assert_eq!(
        rendered,
        "# 生成物。編集は palette.zsh.in を直す。\n\
         \n\
         export FZF_DEFAULT_OPTS='\n\
         \x20 --color=light\n\
         \x20 --color=fg:#d5c4a1,bg:#1d2021,hl:#83a598\n\
         \x20 --color=fg+:#d5c4a1,bg+:#3c3836,hl+:#83a598\n\
         \x20 --color=selected-fg:#d5c4a1,selected-bg:#504945\n\
         \x20 --color=info:#665c54,prompt:#8ec07c,pointer:#83a598\n\
         \x20 --color=marker:#b8bb26,spinner:#83a598\n\
         \x20 --color=border:#504945,header:#8ec07c,gutter:#1d2021\n\
         \x20 --color=preview-bg:#1d2021,preview-fg:#d5c4a1,preview-border:#504945\n\
         '\n"
    );
}

#[test]
fn points_the_terminal_slots_at_the_palette_a_base24_scheme_carries() {
    let theme = theme("base24/dracula.yaml", "dracula");
    let slot = |slot: &str| {
        theme
            .tokens()
            .get(&TokenPath::parse(slot).unwrap())
            .unwrap()
            .to_owned()
    };
    // The bright half comes from base12-base17, which is where base24 departs from base16.
    assert_eq!(slot("ansi.1"), "#ff5555");
    assert_eq!(slot("ansi.9"), "#ff6e6e");
    assert_eq!(slot("ansi.10"), "#69ff94");
    assert_eq!(slot("ansi.11"), "#ffffa5");
    assert_eq!(slot("ansi.12"), "#d6acff");
    assert_eq!(slot("ansi.13"), "#ff92df");
    assert_eq!(slot("ansi.14"), "#a4ffff");
}

#[test]
fn repeats_the_normal_colours_in_the_bright_half_of_a_base16_scheme() {
    let theme = theme("base16/gruvbox-dark-hard.yaml", "gruvbox-dark-hard");
    let slot = |slot: &str| {
        theme
            .tokens()
            .get(&TokenPath::parse(slot).unwrap())
            .unwrap()
            .to_owned()
    };
    assert_eq!(slot("ansi.9"), slot("ansi.1"));
    assert_eq!(slot("ansi.14"), slot("ansi.6"));
}
