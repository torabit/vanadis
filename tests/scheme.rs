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

use vanadis::{System, Template, Theme, ThemeId, TokenPath, Variant, convert, vocabulary};

fn root() -> PathBuf {
    Path::new(env!("CARGO_MANIFEST_DIR")).to_owned()
}

fn fixtures() -> PathBuf {
    root().join("tests/fixtures")
}

/// The converted scheme, written out as a theme file and loaded back.
///
/// `system` is the directory the fixture sits in, which is what `docs/schemes.md` decides
/// the system is. The `system` key inside the file is not read.
fn theme(system: System, scheme: &str, id: &str) -> Theme {
    let bytes = fs::read(fixtures().join("schemes").join(scheme)).unwrap();
    let converted = convert(system, &bytes).unwrap();
    let id = ThemeId::parse(id).unwrap();
    let file =
        vanadis::init::theme(converted.name(), converted.variant(), converted.tokens()).unwrap();
    Theme::parse(&root().join("themes").join(format!("{id}.toml")), &file).unwrap()
}

/// Every fixture scheme, with the system its directory names and the identifier its
/// filename gives the theme it becomes.
fn schemes() -> Vec<(System, &'static str, &'static str)> {
    let mut schemes = vec![
        (
            System::Base16,
            "base16/gruvbox-dark-hard.yaml",
            "gruvbox-dark-hard",
        ),
        (
            System::Base16,
            "base16/solarized-light.yaml",
            "solarized-light",
        ),
        (System::Base24, "base24/dracula.yaml", "dracula"),
        (
            System::Base24,
            "base24/papercolor-light.yaml",
            "papercolor-light",
        ),
    ];
    schemes.extend(tinted8());
    schemes
}

/// The four tinted8 schemes upstream carries: one light and three dark.
fn tinted8() -> Vec<(System, &'static str, &'static str)> {
    vec![
        (
            System::Tinted8,
            "tinted8/catppuccin-latte.yaml",
            "catppuccin-latte",
        ),
        (
            System::Tinted8,
            "tinted8/catppuccin-mocha.yaml",
            "catppuccin-mocha",
        ),
        (System::Tinted8, "tinted8/gruvbox-dark.yaml", "gruvbox-dark"),
        (System::Tinted8, "tinted8/nord.yaml", "nord"),
    ]
}

#[test]
fn fills_the_whole_core_from_every_upstream_scheme() {
    for (system, scheme, id) in schemes() {
        let theme = theme(system, scheme, id);
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
    let theme = theme(
        System::Base16,
        "base16/gruvbox-dark-hard.yaml",
        "gruvbox-dark-hard",
    );
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
    let theme = theme(
        System::Base16,
        "base16/gruvbox-dark-hard.yaml",
        "gruvbox-dark-hard",
    );
    assert_eq!(theme.name(), "Gruvbox dark, hard");
    assert_eq!(theme.variant(), Variant::Dark);
}

#[test]
fn renders_a_core_only_template_against_a_converted_scheme() {
    let theme = theme(
        System::Base16,
        "base16/gruvbox-dark-hard.yaml",
        "gruvbox-dark-hard",
    );
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
    let theme = theme(System::Base24, "base24/dracula.yaml", "dracula");
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
    let theme = theme(
        System::Base16,
        "base16/gruvbox-dark-hard.yaml",
        "gruvbox-dark-hard",
    );
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

/// Every `syntax` and `ui` key a scheme file writes, as the token path it has to become.
///
/// Read off the fixture rather than listed here, so a mapping that carries only some of them
/// fails. A key is a line's text before its first `:`; a line whose remainder opens a quote
/// holds a colour and every other line opens a block. Both spellings a scheme may use — a
/// dotted key and a nested mapping — end at the same path, which is the point of the walk.
/// The colour is taken off the same line, so a mapping that kept every path and collapsed
/// the values onto one colour fails here too.
fn properties(source: &str) -> Vec<(String, String)> {
    let mut properties = Vec::new();
    let mut open: Vec<(usize, String)> = Vec::new();
    let mut block: Option<&str> = None;
    for line in source.lines() {
        let text = line.trim_end();
        let indent = text.len() - text.trim_start().len();
        let text = text.trim();
        if text.is_empty() || text.starts_with('#') {
            continue;
        }
        let (key, rest) = text.split_once(':').unwrap_or((text, ""));
        if indent == 0 {
            open.clear();
            block = ["syntax", "ui"].contains(&key).then_some(key);
            continue;
        }
        let Some(block) = block else {
            continue;
        };
        while open.last().is_some_and(|(at, _)| *at >= indent) {
            open.pop();
        }
        let rest = rest.trim();
        if let Some(rest) = rest.strip_prefix('"') {
            let (colour, _) = rest.split_once('"').unwrap();
            let mut path = vec![block.to_owned()];
            path.extend(open.iter().map(|(_, key)| key.clone()));
            path.push(key.to_owned());
            path.push("default".to_owned());
            properties.push((path.join("."), colour.to_ascii_lowercase()));
        } else {
            open.push((indent, key.to_owned()));
        }
    }
    properties
}

#[test]
fn keeps_every_theming_property_a_tinted8_scheme_writes() {
    for (system, scheme, id) in tinted8() {
        let source = fs::read_to_string(fixtures().join("schemes").join(scheme)).unwrap();
        let properties = properties(&source);
        assert!(
            !properties.is_empty(),
            "{scheme} writes no theming property"
        );

        let theme = theme(system, scheme, id);
        for (property, colour) in properties {
            let path = TokenPath::parse(&property).unwrap();
            assert_eq!(
                theme.tokens().get(&path),
                Some(colour.as_str()),
                "{scheme} {property}"
            );
        }
    }
}

#[test]
fn fills_all_sixteen_terminal_slots_from_a_tinted8_scheme() {
    for (system, scheme, id) in tinted8() {
        let theme = theme(system, scheme, id);
        for slot in 0..16 {
            let path = TokenPath::parse(&format!("ansi.{slot}")).unwrap();
            assert!(theme.tokens().get(&path).is_some(), "{scheme} ansi.{slot}");
        }
    }
}

/// Every core role against the colour `tinted8/gruvbox-dark.yaml` writes.
///
/// The expected column is written out rather than read back off the converter's own table.
/// Each pair is taken from the mapping in `src/scheme/convert/tinted8.rs`, and the comment
/// names the tinted8 key it came through.
#[test]
fn resolves_every_core_role_to_the_colour_the_tinted8_mapping_names() {
    let theme = theme(System::Tinted8, "tinted8/gruvbox-dark.yaml", "gruvbox-dark");
    let expected = [
        ("bg", "#282828"),           // palette.black
        ("fg", "#ebdbb2"),           // palette.white
        ("comment", "#928374"),      // palette.gray-dim
        ("keyword", "#d79921"),      // syntax.keyword
        ("string", "#98971a"),       // palette.green
        ("error", "#cc241d"),        // palette.red
        ("ok", "#98971a"),           // palette.green
        ("warn", "#d79921"),         // palette.yellow
        ("visual", "#d79921"),       // palette.yellow
        ("linenr", "#ebdbb2"),       // palette.white
        ("accent", "#689d6a"),       // palette.cyan
        ("accent-alt", "#458588"),   // palette.blue
        ("accent-warm", "#d65d0e"),  // palette.orange
        ("inactive", "#928374"),     // palette.gray
        ("border", "#928374"),       // palette.gray-dim
        ("selection-bg", "#3c3836"), // palette.black-bright
        ("hover-bg", "#928374"),     // palette.gray-dim
    ];
    assert_eq!(expected.len(), 17, "the core names seventeen roles");

    for (role, colour) in expected {
        let path = TokenPath::parse(&format!("role.{role}")).unwrap();
        assert_eq!(theme.tokens().get(&path), Some(colour), "role.{role}");
    }
}

/// The same mapping against `tinted8/catppuccin-latte.yaml`, which is light.
///
/// `black` is the darkest anchor whatever the variant, so a light scheme takes its
/// background from `white` and its text from `black`, and the palette fallbacks mirror along
/// the luminance scale with them.
///
/// Both fills land on `gray` here. latte writes one surface step, `black-bright`, which
/// mirrors to `white-dim`, and it writes no `white-dim`; a light scheme cannot use a dark
/// scheme's step, and deriving the missing one is outside this format. `gray` is the last
/// link before the required colour, and the required colour is the family root of `bg`, so
/// this is the fill being kept off the background at the cost of sharing `comment`'s grey.
#[test]
fn mirrors_the_tinted8_mapping_for_a_light_scheme() {
    let theme = theme(
        System::Tinted8,
        "tinted8/catppuccin-latte.yaml",
        "catppuccin-latte",
    );
    let expected = [
        ("bg", "#dce0e8"),           // palette.white
        ("fg", "#4c4f69"),           // palette.black
        ("comment", "#9ca0b0"),      // palette.gray, mirrored from gray-dim
        ("keyword", "#8839ef"),      // palette.magenta
        ("string", "#40a02b"),       // palette.green
        ("error", "#d20f39"),        // palette.red
        ("ok", "#40a02b"),           // palette.green
        ("warn", "#df8e1d"),         // palette.yellow
        ("visual", "#df8e1d"),       // palette.yellow
        ("linenr", "#6c6f85"),       // palette.black-bright, mirrored from white-dim
        ("accent", "#179299"),       // palette.cyan
        ("accent-alt", "#1e66f5"),   // palette.blue
        ("accent-warm", "#fe640b"),  // palette.orange
        ("inactive", "#9ca0b0"),     // palette.gray
        ("border", "#9ca0b0"),       // palette.gray, mirrored from gray-dim
        ("selection-bg", "#9ca0b0"), // palette.gray: no surface step mirrors
        ("hover-bg", "#9ca0b0"),     // palette.gray: no surface step mirrors
    ];
    assert_eq!(expected.len(), 17, "the core names seventeen roles");

    for (role, colour) in expected {
        let path = TokenPath::parse(&format!("role.{role}")).unwrap();
        assert_eq!(theme.tokens().get(&path), Some(colour), "role.{role}");
    }
}

/// A role the scheme states as a theming property comes through that property, not through
/// the palette behind it. `tinted8/nord.yaml` states six of them.
#[test]
fn takes_a_core_role_from_the_theming_property_a_tinted8_scheme_states() {
    let theme = theme(System::Tinted8, "tinted8/nord.yaml", "nord");
    let expected = [
        ("comment", "#616e88"),     // syntax.comment
        ("keyword", "#81a1c1"),     // syntax.keyword
        ("string", "#a3be8c"),      // syntax.string
        ("linenr", "#434c5e"),      // ui.gutter.foreground
        ("accent", "#88c0d0"),      // ui.accent.normal
        ("border", "#3b4252"),      // ui.border.normal
        ("hover-bg", "#434c5e"),    // ui.highlight.line.background
        ("inactive", "#616e88"),    // palette.gray, which nord states no property for
        ("accent-warm", "#d08770"), // palette.orange, likewise
    ];
    for (role, colour) in expected {
        let path = TokenPath::parse(&format!("role.{role}")).unwrap();
        assert_eq!(theme.tokens().get(&path), Some(colour), "role.{role}");
    }
}

/// `tinted8/catppuccin-mocha.yaml` writes `black-dim` and no `gray-dim`, so both fills take
/// the recessed surface rather than the mid-scale grey.
///
/// Each is a step from `bg` and neither is the muted foreground, which is the whole point of
/// walking the surfaces before reaching for `gray`. `border` is not a fill and stays in the
/// grey family the specification defaults it to, which is the collision the module doc
/// records.
#[test]
fn takes_a_fill_from_the_surface_step_a_tinted8_scheme_writes() {
    let theme = theme(
        System::Tinted8,
        "tinted8/catppuccin-mocha.yaml",
        "catppuccin-mocha",
    );
    let at = |role: &str| {
        theme
            .tokens()
            .get(&TokenPath::parse(&format!("role.{role}")).unwrap())
            .unwrap()
            .to_owned()
    };
    assert_eq!(at("bg"), "#1e1e2e"); // palette.black
    assert_eq!(at("hover-bg"), "#181825"); // palette.black-dim
    assert_eq!(at("selection-bg"), "#181825"); // palette.black-dim
    assert_eq!(at("border"), "#6c7086"); // palette.gray
    assert_eq!(at("comment"), "#6c7086"); // palette.gray
    assert_ne!(at("hover-bg"), at("bg"));
    assert_ne!(at("hover-bg"), at("comment"));
}

/// `tinted8/nord.yaml` writes no `black-*` variant at all, so `selection-bg` runs out of
/// surfaces and takes `gray` on the last link before the required colour.
///
/// The required colour is `black`, which is what `bg` is built from, so without that link
/// the selection would be the colour of the background it is drawn on.
#[test]
fn takes_a_grey_for_a_fill_a_tinted8_scheme_offers_no_surface_for() {
    let theme = theme(System::Tinted8, "tinted8/nord.yaml", "nord");
    let at = |role: &str| {
        theme
            .tokens()
            .get(&TokenPath::parse(&format!("role.{role}")).unwrap())
            .unwrap()
            .to_owned()
    };
    assert_eq!(at("bg"), "#2e3440"); // palette.black
    assert_eq!(at("selection-bg"), "#616e88"); // palette.gray
    assert_ne!(at("selection-bg"), at("bg"));
}

/// The sixteen slots against the palette `tinted8/gruvbox-dark.yaml` writes in full.
#[test]
fn points_the_terminal_slots_at_the_palette_a_tinted8_scheme_carries() {
    let theme = theme(System::Tinted8, "tinted8/gruvbox-dark.yaml", "gruvbox-dark");
    let expected = [
        "#282828", // black
        "#cc241d", // red
        "#98971a", // green
        "#d79921", // yellow
        "#458588", // blue
        "#b16286", // magenta
        "#689d6a", // cyan
        "#ebdbb2", // white
        "#3c3836", // black-bright
        "#fb4934", // red-bright
        "#b8bb26", // green-bright
        "#fabd2f", // yellow-bright
        "#83a598", // blue-bright
        "#d3869b", // magenta-bright
        "#8ec07c", // cyan-bright
        "#fbf1c7", // white-bright
    ];
    for (slot, colour) in expected.into_iter().enumerate() {
        let path = TokenPath::parse(&format!("ansi.{slot}")).unwrap();
        assert_eq!(theme.tokens().get(&path), Some(colour), "ansi.{slot}");
    }
}

/// `tinted8/catppuccin-latte.yaml` writes one bright variant and leaves the other seven to
/// the builder, so seven of the bright slots repeat the hue they brighten.
#[test]
fn repeats_the_hue_in_a_bright_slot_a_tinted8_scheme_leaves_out() {
    let theme = theme(
        System::Tinted8,
        "tinted8/catppuccin-latte.yaml",
        "catppuccin-latte",
    );
    let slot = |slot: usize| {
        theme
            .tokens()
            .get(&TokenPath::parse(&format!("ansi.{slot}")).unwrap())
            .unwrap()
            .to_owned()
    };
    assert_eq!(slot(8), "#6c6f85", "palette.black-bright is written");
    for bright in 9..16 {
        assert_eq!(slot(bright), slot(bright - 8), "ansi.{bright}");
    }
}

#[test]
fn keeps_the_name_and_variant_a_tinted8_scheme_states() {
    let nord = theme(System::Tinted8, "tinted8/nord.yaml", "nord");
    assert_eq!(nord.name(), "Nord");
    assert_eq!(nord.variant(), Variant::Dark);
}

/// Neither catppuccin scheme writes `scheme.name`, so the display name is `scheme.family`
/// joined to `scheme.style`, which is what the builder specification resolves it to.
#[test]
fn builds_a_tinted8_display_name_from_the_family_and_the_style() {
    let latte = theme(
        System::Tinted8,
        "tinted8/catppuccin-latte.yaml",
        "catppuccin-latte",
    );
    assert_eq!(latte.name(), "Catppuccin Latte");
    assert_eq!(latte.variant(), Variant::Light);
}
