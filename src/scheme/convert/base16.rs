//! base16 and base24, which share a palette and differ only in how wide it is.
//!
//! The `base00`-`base0F` mapping onto `[role]` is decided by
//! `docs/core-vocabulary.md#base16-onto-the-core` and the sixteen terminal slots by
//! `docs/theme-format.md#a-base16-scheme-in-this-format`. Neither table is re-argued here.
//!
//! # Where base24 departs
//!
//! base24's styling specification (`tinted-theming/base24`, `styling.md` v0.1.3) shares
//! `base00`-`base0F` with base16 and adds eight slots. Its ANSI column gives six of them a
//! terminal meaning: `base12` Bright Red, `base13` Bright Yellow, `base14` Bright Green,
//! `base15` Bright Cyan, `base16` Bright Blue and `base17` Bright Magenta. Those take slots
//! 9 through 14, which is the whole departure: base16 has no separate bright set, so it
//! repeats slots 1 through 6 there.
//!
//! The other two, `base10` and `base11`, are a darker and a darkest background. The
//! specification gives them no ANSI slot and `[ansi]` stays exactly sixteen, so they land in
//! `[colors]` and nothing points at them. They fill no core token either: `role.hover-bg`
//! and `role.bg` are already filled from `base01` and `base00`, and
//! `docs/core-vocabulary.md` says a wider system's converter may not be blocked by a core
//! token base16 already fills.
//!
//! The specification also publishes a fallback table from base24 back to base16
//! (`base12` to `base08`, `base13` to `base0A`, and so on). That is how a base16 scheme is
//! read through a base24 template, and it is exactly what [`Family::source`] applies for
//! base16. It is not applied to a scheme converted as base24 that then omits a slot: such a
//! file is malformed, and quietly giving it base16's colour would produce a theme whose
//! bright colours silently equal its normal ones.

use std::collections::BTreeMap;
use std::fmt;

use saphyr::Yaml;

use super::{ConvertError, Converted, hex, literal, variant};
use crate::scheme::{System, yaml};
use crate::token::TokenPath;

/// Which member of the base16 family a scheme is written for.
///
/// Not [`System`](crate::scheme::System), which names a directory in the collection and
/// carries tinted8 as well. This one is the two widths of palette this module reads, and
/// growing a third variant onto it would hand this module a scheme it cannot convert.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Family {
    /// base16: `base00` through `base0F`.
    Base16,
    /// base24: base16's sixteen, plus `base10` through `base17`.
    Base24,
}

/// The sixteen slots base16 fills, spelled the way the styling specification spells them.
const BASE16: [&str; 16] = [
    "base00", "base01", "base02", "base03", "base04", "base05", "base06", "base07", "base08",
    "base09", "base0A", "base0B", "base0C", "base0D", "base0E", "base0F",
];

/// The eight slots base24 adds, and what each falls back to in a base16 scheme.
const EXTENDED: [(&str, &str); 8] = [
    ("base10", "base00"),
    ("base11", "base00"),
    ("base12", "base08"),
    ("base13", "base0A"),
    ("base14", "base0B"),
    ("base15", "base0C"),
    ("base16", "base0D"),
    ("base17", "base0E"),
];

/// The terminal's sixteen slots, in order, as the base24 styling specification's ANSI
/// column names them. A base16 scheme reaches the same table through [`Family::source`].
const ANSI: [&str; 16] = [
    "base00", "base08", "base0B", "base0A", "base0D", "base0E", "base0C", "base05", "base03",
    "base12", "base14", "base13", "base16", "base17", "base15", "base07",
];

/// The slot both families put the default background in, which is what the variant falls
/// back to reading when the scheme does not state one.
const BACKGROUND: &str = "base00";

/// The seventeen core `[role]` tokens and the slot each takes.
const ROLES: [(&str, &str); 17] = [
    ("bg", "base00"),
    ("fg", "base05"),
    ("comment", "base03"),
    ("keyword", "base0E"),
    ("string", "base0B"),
    ("error", "base08"),
    ("ok", "base0B"),
    ("warn", "base0A"),
    ("visual", "base0D"),
    ("linenr", "base04"),
    ("accent", "base0D"),
    ("accent-alt", "base0C"),
    ("accent-warm", "base09"),
    ("inactive", "base03"),
    ("border", "base02"),
    ("selection-bg", "base02"),
    ("hover-bg", "base01"),
];

impl Family {
    /// The system a scheme of this family is filed under.
    ///
    /// One to one, and it exists so an error names what the caller asked for rather than
    /// the discriminator this module reads the palette with. tinted8 has no family and
    /// reports the same error through the same variant.
    fn system(self) -> System {
        match self {
            Self::Base16 => System::Base16,
            Self::Base24 => System::Base24,
        }
    }

    /// The slots a scheme of this family has to carry.
    fn slots(self) -> Vec<&'static str> {
        let extended = EXTENDED.iter().map(|(slot, _)| *slot);
        match self {
            Self::Base16 => BASE16.to_vec(),
            Self::Base24 => BASE16.into_iter().chain(extended).collect(),
        }
    }

    /// Which slot of this family supplies `slot`.
    ///
    /// base24 supplies its own. base16 does not carry `base10` through `base17` at all, so
    /// they resolve through the fallback table the base24 specification publishes.
    fn source(self, slot: &'static str) -> &'static str {
        match self {
            Self::Base24 => slot,
            Self::Base16 => EXTENDED
                .iter()
                .find(|(extended, _)| *extended == slot)
                .map_or(slot, |(_, fallback)| *fallback),
        }
    }
}

impl fmt::Display for Family {
    fn fmt(&self, f: &mut fmt::Formatter<'_>) -> fmt::Result {
        f.write_str(match self {
            Self::Base16 => "base16",
            Self::Base24 => "base24",
        })
    }
}

/// Converts `document`, a scheme of `family`, into the theme model.
///
/// `[colors]` takes the palette under the slot names, lowercased, and everything else points
/// at it: `docs/theme-format.md` makes `[colors]` the primitives a converter fills first.
///
/// # Errors
///
/// Returns [`ConvertError`] when the scheme omits `name` or `palette`, omits a slot `family`
/// requires, writes a variant that is neither `dark` nor `light`, or writes a palette entry
/// that is not `#rrggbb`.
pub(super) fn convert(document: &Yaml<'_>, family: Family) -> Result<Converted, ConvertError> {
    let name = yaml::required(document, "name")?.to_owned();
    let palette = yaml::nested(document, "palette")?;

    let mut tokens = BTreeMap::new();
    for slot in family.slots() {
        tokens.insert(
            TokenPath::from_segments(["colors", &segment(slot)]),
            literal(colour(palette, family, slot)?),
        );
    }

    for (index, slot) in ANSI.into_iter().enumerate() {
        tokens.insert(
            TokenPath::from_segments(["ansi", &index.to_string()]),
            reference(family.source(slot)),
        );
    }
    // No role names an extended slot, so no role reaches the fallback table: every slot in
    // `ROLES` is one both families carry under its own name.
    for (role, slot) in ROLES {
        tokens.insert(TokenPath::from_segments(["role", role]), reference(slot));
    }
    if let Some(author) = yaml::optional(document, "author")? {
        tokens.insert(
            TokenPath::from_segments(["meta", "author"]),
            author.to_owned(),
        );
    }

    let background = colour(palette, family, BACKGROUND)?;
    let variant = variant(yaml::optional(document, "variant")?, background)?;

    Ok(Converted {
        name,
        variant,
        tokens,
    })
}

/// The colour `slot` holds, as red, green and blue.
///
/// The styling specifications spell `base0A` with an uppercase letter and every upstream
/// scheme follows them, so that spelling is looked up first. A scheme that writes `base0a`
/// still reads: the two spell one slot, and refusing the second would be pedantry the file
/// cannot act on.
///
/// # Errors
///
/// Returns [`ConvertError::Slot`] when the palette does not write `slot` and
/// [`ConvertError::Hex`] when it does not hold `#rrggbb`.
fn colour(palette: &Yaml<'_>, family: Family, slot: &str) -> Result<[u8; 3], ConvertError> {
    let written = match yaml::optional(palette, slot)? {
        Some(written) => Some(written),
        None => yaml::optional(palette, &segment(slot))?,
    };
    let written = written.ok_or_else(|| ConvertError::Slot {
        system: family.system(),
        slot: slot.to_owned(),
    })?;
    hex(slot, written)
}

/// `slot` as the token segment it becomes: `base0A` is not one, `base0a` is.
fn segment(slot: &str) -> String {
    slot.to_ascii_lowercase()
}

/// `slot` as the whole-string reference a theme file points at `[colors]` with.
fn reference(slot: &str) -> String {
    format!("{{{{colors.{}}}}}", segment(slot))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Theme;
    use crate::scheme::{Problem, System};
    use crate::theme::Variant;
    use crate::vocabulary;
    use std::fmt::Write as _;
    use std::path::Path;

    /// A base16 scheme whose palette is a distinct colour per slot.
    fn base16() -> String {
        scheme("base16", 16)
    }

    /// A base24 scheme whose palette is a distinct colour per slot.
    fn base24() -> String {
        scheme("base24", 24)
    }

    /// `count` slots, each holding a colour that names its own index.
    ///
    /// The `system` key is written because upstream files write it. Nothing reads it: the
    /// system every test converts under is the one it passes in.
    fn scheme(system: &str, count: usize) -> String {
        let mut text = format!("system: \"{system}\"\nname: \"Test\"\npalette:\n");
        for index in 0..count {
            let _ = writeln!(text, "  base{index:02X}: \"#0000{index:02x}\"");
        }
        text
    }

    fn convert(system: System, text: &str) -> Result<Converted, ConvertError> {
        super::super::convert(system, text.as_bytes())
    }

    fn token(system: System, text: &str, path: &str) -> String {
        convert(system, text)
            .unwrap()
            .tokens
            .get(&TokenPath::parse(path).unwrap())
            .unwrap_or_else(|| panic!("{path} is not defined"))
            .clone()
    }

    /// The converted scheme, emitted and loaded back, which is what resolves the references.
    fn loaded(system: System, text: &str) -> Theme {
        let converted = convert(system, text).unwrap();
        let file =
            crate::init::theme(converted.name(), converted.variant(), converted.tokens()).unwrap();
        Theme::parse(Path::new("themes/test.toml"), &file).unwrap()
    }

    #[test]
    fn fills_every_core_token_from_a_base16_scheme() {
        assert_eq!(
            vocabulary::missing(loaded(System::Base16, &base16()).tokens()),
            Vec::new()
        );
    }

    #[test]
    fn fills_every_core_token_from_a_base24_scheme() {
        assert_eq!(
            vocabulary::missing(loaded(System::Base24, &base24()).tokens()),
            Vec::new()
        );
    }

    #[test]
    fn takes_the_sixteen_palette_entries_into_colors() {
        assert_eq!(token(System::Base16, &base16(), "colors.base00"), "#000000");
        assert_eq!(token(System::Base16, &base16(), "colors.base0f"), "#00000f");
    }

    #[test]
    fn lowercases_a_slot_name_into_a_token_segment() {
        assert_eq!(token(System::Base16, &base16(), "colors.base0a"), "#00000a");
    }

    #[test]
    fn keeps_the_extended_base24_slots_in_colors() {
        assert_eq!(token(System::Base24, &base24(), "colors.base10"), "#000010");
        assert_eq!(token(System::Base24, &base24(), "colors.base17"), "#000017");
    }

    #[test]
    fn leaves_a_base16_scheme_without_the_extended_slots() {
        assert!(
            !convert(System::Base16, &base16())
                .unwrap()
                .tokens
                .contains_key(&TokenPath::parse("colors.base10").unwrap())
        );
    }

    #[test]
    fn points_a_role_at_the_colours_table_rather_than_repeating_the_hex() {
        assert_eq!(
            token(System::Base16, &base16(), "role.bg"),
            "{{colors.base00}}"
        );
    }

    #[test]
    fn repeats_the_normal_colours_in_the_bright_half_of_a_base16_scheme() {
        for (bright, normal) in [(9, 1), (10, 2), (11, 3), (12, 4), (13, 5), (14, 6)] {
            let theme = loaded(System::Base16, &base16());
            let slot = |index: usize| {
                theme
                    .tokens()
                    .get(&TokenPath::from_segments(["ansi", &index.to_string()]))
                    .map(ToOwned::to_owned)
            };
            assert_eq!(slot(bright), slot(normal), "ansi.{bright}");
        }
    }

    #[test]
    fn takes_the_bright_half_of_a_base24_scheme_from_the_extended_slots() {
        let bright = |path: &str| token(System::Base24, &base24(), path);
        assert_eq!(bright("ansi.9"), "{{colors.base12}}");
        assert_eq!(bright("ansi.10"), "{{colors.base14}}");
        assert_eq!(bright("ansi.11"), "{{colors.base13}}");
        assert_eq!(bright("ansi.12"), "{{colors.base16}}");
        assert_eq!(bright("ansi.13"), "{{colors.base17}}");
        assert_eq!(bright("ansi.14"), "{{colors.base15}}");
    }

    #[test]
    fn keeps_the_dark_and_light_ends_of_both_families_the_same() {
        let end = |path: &str| token(System::Base24, &base24(), path);
        assert_eq!(end("ansi.0"), "{{colors.base00}}");
        assert_eq!(end("ansi.8"), "{{colors.base03}}");
        assert_eq!(end("ansi.15"), "{{colors.base07}}");
    }

    #[test]
    fn takes_the_display_name_across_verbatim() {
        let text = base16().replace("\"Test\"", "\"Gruvbox dark, hard\"");
        assert_eq!(
            convert(System::Base16, &text).unwrap().name(),
            "Gruvbox dark, hard"
        );
    }

    #[test]
    fn takes_the_author_across_as_provenance() {
        let text = format!("author: \"morhetz\"\n{}", base16());
        assert_eq!(token(System::Base16, &text, "meta.author"), "morhetz");
    }

    #[test]
    fn leaves_the_author_out_when_the_scheme_does_not_carry_one() {
        assert!(
            !convert(System::Base16, &base16())
                .unwrap()
                .tokens
                .contains_key(&TokenPath::parse("meta.author").unwrap())
        );
    }

    #[test]
    fn takes_the_variant_from_the_scheme_metadata() {
        let text = format!("variant: \"light\"\n{}", base16());
        assert_eq!(
            convert(System::Base16, &text).unwrap().variant(),
            Variant::Light
        );
    }

    #[test]
    fn infers_the_variant_from_base00_when_the_scheme_omits_it() {
        assert_eq!(
            convert(System::Base16, &base16()).unwrap().variant(),
            Variant::Dark
        );
    }

    #[test]
    fn stores_a_palette_entry_in_lowercase() {
        let text = base16().replace("\"#000000\"", "\"#1D2021\"");
        assert_eq!(token(System::Base16, &text, "colors.base00"), "#1d2021");
    }

    /// `docs/schemes.md` gives the system to the caller: the directory the file came out of
    /// decides it, and the file's own `system` key is not read. Converting a file that says
    /// `base24` as base16 has to produce a base16 theme.
    #[test]
    fn takes_the_system_from_the_caller_and_not_from_the_file() {
        assert_eq!(
            token(System::Base16, &base24(), "ansi.9"),
            "{{colors.base08}}"
        );
        assert!(
            !convert(System::Base16, &base24())
                .unwrap()
                .tokens
                .contains_key(&TokenPath::parse("colors.base10").unwrap())
        );
    }

    #[test]
    fn converts_a_scheme_that_declares_no_system_at_all() {
        let text = base16().replace("system: \"base16\"\n", "");
        assert_eq!(token(System::Base16, &text, "colors.base00"), "#000000");
    }

    #[test]
    fn reports_a_base16_scheme_missing_a_slot() {
        let text = base16().replace("  base0F: \"#00000f\"\n", "");
        assert!(matches!(
            convert(System::Base16, &text),
            Err(ConvertError::Slot { ref slot, .. }) if slot == "base0F"
        ));
    }

    #[test]
    fn reports_a_base24_scheme_missing_an_extended_slot() {
        let text = base24().replace("  base12: \"#000012\"\n", "");
        assert!(matches!(
            convert(System::Base24, &text),
            Err(ConvertError::Slot { ref slot, .. }) if slot == "base12"
        ));
    }

    #[test]
    fn names_the_family_the_missing_slot_was_required_by() {
        let text = base24().replace("  base12: \"#000012\"\n", "");
        let error = convert(System::Base24, &text).unwrap_err();
        assert!(error.to_string().contains("base24"), "{error}");
    }

    #[test]
    fn reports_a_scheme_without_a_name() {
        let text = base16().replace("name: \"Test\"\n", "");
        assert!(matches!(
            convert(System::Base16, &text),
            Err(ConvertError::Invalid {
                source: Problem::Missing("name")
            })
        ));
    }

    #[test]
    fn reports_a_scheme_without_a_palette() {
        assert!(matches!(
            convert(System::Base16, "system: \"base16\"\nname: \"Test\"\n"),
            Err(ConvertError::Invalid {
                source: Problem::Missing("palette")
            })
        ));
    }

    #[test]
    fn reports_a_palette_entry_that_is_not_a_hex_colour() {
        let text = base16().replace("\"#000000\"", "\"NONE\"");
        assert!(matches!(
            convert(System::Base16, &text),
            Err(ConvertError::Hex { ref slot, .. }) if slot == "base00"
        ));
    }

    #[test]
    fn reports_a_variant_that_is_neither_dark_nor_light() {
        let text = format!("variant: \"pale\"\n{}", base16());
        assert!(matches!(
            convert(System::Base16, &text),
            Err(ConvertError::Invalid {
                source: Problem::Variant(_)
            })
        ));
    }
}
