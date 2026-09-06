//! tinted8, which separates a palette of hues from the `syntax` and `ui` roles drawn on it.
//!
//! The behaviour here is decided by tinted8's styling specification
//! (`tinted-theming/home`, `specs/tinted8/styling.md` v0.2.0-beta11) and, where that
//! document defers, by its builder specification (`specs/tinted8/builder.md`, same version).
//! `docs/theme-format.md` decides where the result lands: it already states that reading a
//! scheme means "dropping its entries into `[colors]` under their own names and pointing
//! `[ansi]` and `[role]` at them", and that is what this does.
//!
//! # The four blocks
//!
//! `scheme` holds the metadata and is the only block base16 does not have an equivalent of.
//! `scheme.system` is where a tinted8 file declares its system, and it is not read:
//! `docs/schemes.md` decides that the system is the caller's, taken from the directory the
//! file sits in. `[meta]` has a home for two of the block's keys: the display name and
//! `scheme.author`. The
//! specification lets a scheme write `name`, `slug`, or `family` and `style`, and requires
//! at least one; the name is resolved in that order, as
//! `builder.md#name-and-slug-handling` states it. `theme-author`, `description`,
//! `supports.styling-spec`, `slug` and `family` carry no further, because `docs/theme-format.md`
//! makes an unknown `[meta]` key an error and `[text]` is a namespace a converter leaves
//! empty.
//!
//! `variant` is required and is read strictly. The shared luminance fallback in the parent
//! module is for base16, whose specification leaves `variant` optional; a tinted8 scheme
//! that omits it is malformed, and guessing would also be circular here, since which anchor
//! is the background is exactly what the variant decides.
//!
//! `palette` is eight required hues plus optional `-bright` and `-dim` variants and three
//! supplemental hues. The specification requires these to be written as flat hyphenated
//! keys (`red-bright`), never as a nested `red: { bright: ... }` mapping, so the key a
//! scheme writes is already a token segment and `[colors]` takes it verbatim. Accepting the
//! nested spelling as well was rejected: the specification's compliance section rules it
//! out, and reading a file the specification calls malformed would put two spellings of one
//! palette into the theme.
//!
//! `syntax` and `ui` are the theming properties, and neither has a fixed shape: upstream
//! schemes write `entity.name.function` as a dotted key and `indent-guide:` as a nested
//! mapping, sometimes in the same file. Both are read into the same token path, so a scheme
//! keeps its meaning whichever way it spells it.
//!
//! # Where a theming property lands
//!
//! A theming property becomes a token under its own path plus `default`:
//! `syntax.entity.name` becomes `syntax.entity.name.default`. The suffix is not decoration.
//! A scheme may set both `syntax.constant` and `syntax.constant.numeric`, and every upstream
//! tinted8 scheme but one does; a theme file cannot hold `constant` as both a string and a
//! table, so one of the two would have to be dropped. `.default` is upstream's own answer to
//! the same collision (`builder.md#the-default-suffix`), which is the reason to prefer it
//! over flattening the scope into a single hyphenated segment: a template written against
//! the builder's variable names reaches the same path here, minus the colour suffix.
//!
//! Because the encoding spends that segment, `default` is reserved: a property key carrying
//! it is [`ConvertError::Reserved`]. A scheme writing both `syntax.constant` and
//! `syntax.constant.default` would otherwise convert and then emit `syntax.constant.default`
//! as a colour and again as a table, which is a theme file that does not load. No key in the
//! specification spells it, so only the unknown keys this converter deliberately passes
//! through can reach it, which is what makes closing it this module's business.
//!
//! Theming properties hold the colour the scheme writes rather than a reference into
//! `[colors]`. They are frequently colours the palette does not carry at all, and
//! `docs/theme-format.md` states that the primitives convention is not enforced.
//!
//! # The sixteen terminal slots
//!
//! The ANSI Mapping column of the styling specification's two palette tables gives them
//! directly: slots 0 to 7 are the eight required hues in the order `black`, `red`, `green`,
//! `yellow`, `blue`, `magenta`, `cyan`, `white`, and slots 8 to 15 are the `-bright` variant
//! of each. The mapping is fixed and does not mirror between variants: `ansi.0` is the black
//! of the palette, not the background of the theme.
//!
//! A `-bright` variant is optional, and a builder derives the ones a scheme leaves out. This
//! converter does not derive colour, so a missing `-bright` falls back to the unsuffixed
//! key, which the specification calls the `normal` variant of the same hue and which the
//! eight required colours guarantee is present. base16 reaches the same place from the other
//! direction, repeating slots 1 to 6 in 9 to 14 because it has no bright set at all.
//!
//! # The core onto tinted8
//!
//! `docs/core-vocabulary.md` requires a converter to fill all seventeen `[role]` tokens.
//! Each takes the first source it has: the theming property named below, when the scheme
//! writes it; then a palette key, when the scheme writes that; and last a required colour,
//! which every scheme carries.
//!
//! | role | theming property | palette | required |
//! | --- | --- | --- | --- |
//! | `bg` | `ui.global.normal.background` | — | `black` |
//! | `fg` | `ui.global.normal.foreground` | — | `white` |
//! | `comment` | `syntax.comment` | `gray-dim`, `gray` | `white` |
//! | `keyword` | `syntax.keyword` | — | `magenta` |
//! | `string` | `syntax.string` | — | `green` |
//! | `error` | `ui.status.error` | — | `red` |
//! | `ok` | `ui.status.success` | — | `green` |
//! | `warn` | `ui.status.warning` | — | `yellow` |
//! | `visual` | `ui.highlight.search.foreground` | — | `yellow` |
//! | `linenr` | `ui.gutter.foreground` | `white-dim` | `white` |
//! | `accent` | `ui.accent.normal` | — | `cyan` |
//! | `accent-alt` | — | — | `blue` |
//! | `accent-warm` | — | `orange` | `yellow` |
//! | `inactive` | — | `gray` | `white` |
//! | `border` | `ui.border.normal` | `gray-dim`, `gray` | `black` |
//! | `selection-bg` | `ui.selection.background` | `black-bright`, `black-dim`, `gray` | `black` |
//! | `hover-bg` | `ui.highlight.line.background` | `gray-dim`, `black-bright`, `black-dim`, `gray` | `black` |
//!
//! Every property in that column is one the styling specification states the meaning of, and
//! the palette and required columns are the builder's own default for that property, cut
//! short at a colour the scheme certainly carries. Where the builder would compute one
//! (`gray` is the midpoint of `black` and `white`, `orange` is a hue-shifted `yellow`) the
//! column names the colour it would compute from instead.
//!
//! A palette key is tried exactly as written, and each chain says every step it takes. The
//! two fills are why. `gray` is the midpoint of `black` and `white`, so a chain that walked
//! from `gray-dim` to `gray` on its own would move a fill a full step towards the
//! foreground: `catppuccin-mocha` would draw the row under the cursor in `#6c7086` on a
//! `#1e1e2e` background, which is neither one step from `bg` nor distinguishable from its
//! own `comment`. `hover-bg` and `selection-bg` therefore walk down the surfaces the palette
//! carries, `black-bright` then `black-dim`, before they will take a grey.
//!
//! `border` is not one of them. It colours pane borders, box drawing and separators, which
//! are drawn over the background rather than as one, and the specification puts its default
//! in the grey family with `syntax.comment` rather than in the black family with
//! `ui.selection.background`. `docs/core-vocabulary.md` asks "one step from `bg`" of
//! `hover-bg` and of nothing else. So `border` stays on `gray-dim` then `gray`, and
//! `comment` and `inactive` keep `gray` too, which is what a muted foreground wants.
//!
//! The two fills end `gray` before the required colour rather than at it. The required
//! colour is `black`, which is the family root of `bg` itself, so a fill that fell that far
//! would equal the background it is drawn on. The rule the three chains encode: a real
//! surface step beats a grey, and a grey beats a colour identical to the background. Their
//! heads differ because the specification's default families differ; their tails agree
//! because that rule does not depend on which family a fill started in.
//!
//! Three roles take no theming property. `accent-alt` and `accent-warm` have none to take:
//! `ui.link.normal.foreground` is the only other accent-shaped key and its default is the
//! same cyan `ui.accent.normal` has, so reading it would make the two accents equal in every
//! scheme that leaves both to the builder. `inactive` takes `gray` directly, one step
//! brighter than the `gray-dim` `comment` falls to, which is the only distinction between
//! dimmed and commented text a tinted8 palette offers.
//!
//! `visual` takes `ui.highlight.search.foreground` because
//! `docs/core-vocabulary.md` defines the role as "a highlight drawn over text: search
//! matches, hints, selected entries", and that key is the one the styling specification
//! gives the same meaning to. Two other candidates were considered and lose.
//! `ui.accent.normal` is what base16 reaches by analogy, and it collides with `accent`.
//! `ui.highlight.text.foreground` reads closer still, and its builder default is the
//! foreground itself, so it would collide with `role.fg` and paint a highlight the colour of
//! ordinary text, which is worse than either.
//!
//! Collisions remain, and they are arithmetic rather than mapping errors, as they are for
//! base16. `string` and `ok` share green, and `visual` and `warn` share yellow in a scheme
//! that states neither property.
//!
//! `border` and `comment` share a grey in any scheme that does not state
//! `ui.border.normal`, which is the same shape as base16's `base03` carrying both `comment`
//! and `inactive`: a palette with one grey family cannot separate a separator from the text
//! beside it. `gruvbox-dark` shows it at its widest, with `border`, `hover-bg` and `comment`
//! all `#928374`. The cause is in the scheme, not here: it writes one value for two steps of
//! the scale, `gray` and `gray-dim` both `#928374`, and it states no
//! `ui.highlight.line.background`, so the builder default `gray-dim` is what every
//! conforming tinted8 builder puts behind the cursor row too. Demoting `gray-dim` below
//! `black-bright` would override a normative default with a preference, which
//! `docs/theme-format.md` puts outside what this converter does.
//!
//! A fill lands on a grey when the palette offers no surface step in the direction the
//! variant needs. `nord` writes no `black-*` variant at all and `catppuccin-latte` writes
//! `black-bright`, which a light scheme mirrors to `white-dim`, which it does not write; in
//! both, `selection-bg` takes `gray` and so equals `comment`. That is the trade the tail
//! makes, and it is the better half of it: a selection the colour of the background cannot
//! be seen at all, while one the colour of comment text still shows every line that is not
//! a comment.
//!
//! # Mirroring a light scheme
//!
//! `palette.black` is the darkest anchor and `palette.white` the lightest, whatever the
//! variant, so in a light scheme the background is `white` and the text is `black`. The
//! builder specification states the rule as a nine-step luminance scale from `black-dim` to
//! `white-bright`, mirrored by index, and the palette column above is written for a dark
//! scheme and mirrored through it for a light one. `gray` mirrors onto itself and a hue is
//! untouched.

use std::collections::BTreeMap;

use saphyr::Yaml;

use super::{ConvertError, Converted, hex, literal};
use crate::scheme::{Problem, System, yaml};
use crate::theme::Variant;
use crate::token::{TokenPath, is_segment};

/// The eight colours every tinted8 scheme carries, in the order the ANSI Mapping column of
/// the styling specification puts them, which is also terminal slots 0 to 7.
const REQUIRED: [&str; 8] = [
    "black", "red", "green", "yellow", "blue", "magenta", "cyan", "white",
];

/// The luminance scale the builder specification mirrors a light scheme through, darkest
/// first. A name in it mirrors to the name eight places from its own index.
const SCALE: [&str; 9] = [
    "black-dim",
    "black",
    "black-bright",
    "gray-dim",
    "gray",
    "gray-bright",
    "white-dim",
    "white",
    "white-bright",
];

/// The two blocks of theming properties, which are read the same way.
const BLOCKS: [&str; 2] = ["syntax", "ui"];

/// The leaf a theming property's own colour is written under, which this converter reserves.
///
/// A scheme key carrying this segment would claim a token path the encoding has already
/// spoken for: a scheme writing both `syntax.constant` and `syntax.constant.default` emits
/// `syntax.constant.default` as a colour and again as a table, which is a theme file that
/// does not load. `docs/theme-format.md` makes a TOML table a namespace and a string a
/// token, and one key cannot be both.
const DEFAULT: &str = "default";

/// The seventeen core `[role]` tokens: the theming property each prefers, the palette keys
/// it falls to in order, and the required colour it ends at. Written for a dark scheme.
///
/// A palette key here is tried exactly as written. A chain that means "this variant, then
/// the hue behind it" says both, because a chain that walked to the unsuffixed key by itself
/// could not put anything between the two.
const ROLES: [(&str, Option<&str>, &[&str], &str); 17] = [
    ("bg", Some("ui.global.normal.background"), &[], "black"),
    ("fg", Some("ui.global.normal.foreground"), &[], "white"),
    (
        "comment",
        Some("syntax.comment"),
        &["gray-dim", "gray"],
        "white",
    ),
    ("keyword", Some("syntax.keyword"), &[], "magenta"),
    ("string", Some("syntax.string"), &[], "green"),
    ("error", Some("ui.status.error"), &[], "red"),
    ("ok", Some("ui.status.success"), &[], "green"),
    ("warn", Some("ui.status.warning"), &[], "yellow"),
    (
        "visual",
        Some("ui.highlight.search.foreground"),
        &[],
        "yellow",
    ),
    (
        "linenr",
        Some("ui.gutter.foreground"),
        &["white-dim"],
        "white",
    ),
    ("accent", Some("ui.accent.normal"), &[], "cyan"),
    ("accent-alt", None, &[], "blue"),
    ("accent-warm", None, &["orange"], "yellow"),
    ("inactive", None, &["gray"], "white"),
    (
        "border",
        Some("ui.border.normal"),
        &["gray-dim", "gray"],
        "black",
    ),
    (
        "selection-bg",
        Some("ui.selection.background"),
        &["black-bright", "black-dim", "gray"],
        "black",
    ),
    (
        "hover-bg",
        Some("ui.highlight.line.background"),
        &["gray-dim", "black-bright", "black-dim", "gray"],
        "black",
    ),
];

/// Converts `document`, a tinted8 scheme, into the theme model.
///
/// # Errors
///
/// Returns [`ConvertError`] when the scheme omits `scheme`, `variant` or `palette`, names
/// itself with none of `scheme.name`, `scheme.slug` and `scheme.family`, omits one of the
/// eight required palette colours, writes a variant that is neither `dark` nor `light`,
/// writes a key that cannot become a token segment or that uses the reserved `default`
/// segment, writes one theming property twice, or writes a colour that is not `#rrggbb`.
pub(super) fn convert(document: &Yaml<'_>) -> Result<Converted, ConvertError> {
    let scheme = yaml::nested(document, "scheme")?;
    let name = name(scheme)?;
    let variant = variant(document)?;
    let palette = yaml::nested(document, "palette")?;

    for required in REQUIRED {
        if !yaml::writes(palette, required) {
            return Err(ConvertError::Slot {
                system: System::Tinted8,
                slot: required.to_owned(),
            });
        }
    }

    // A palette key is flat, so two of them reach one `[colors]` token only by being the
    // same key, and the parser folds those into the entry written last before this sees
    // them. There is nothing here for the guard `properties` needs to hold, which is why
    // this inserts without one. `yaml::tests::folds_a_key_the_scheme_writes_twice` pins it.
    let mut tokens = BTreeMap::new();
    for (key, value) in yaml::entries(palette, "palette")? {
        let at = format!("palette.{key}");
        if !is_segment(key) {
            return Err(ConvertError::Segment {
                key: at,
                segment: key.to_owned(),
            });
        }
        let written = value.as_str().ok_or_else(|| Problem::Type {
            key: at.clone(),
            expected: "a string",
        })?;
        tokens.insert(
            TokenPath::from_segments(["colors", key]),
            literal(hex(&at, written)?),
        );
    }

    for block in BLOCKS {
        if yaml::writes(document, block) {
            let mut path = vec![block.to_owned()];
            properties(yaml::nested(document, block)?, &mut path, &mut tokens)?;
        }
    }

    for (index, required) in (0..16).zip(REQUIRED.into_iter().cycle()) {
        let slot = if index < 8 {
            required.to_owned()
        } else {
            format!("{required}-bright")
        };
        let source = source(palette, &slot).unwrap_or_else(|| required.to_owned());
        tokens.insert(
            TokenPath::from_segments(["ansi", &index.to_string()]),
            reference(&format!("colors.{source}")),
        );
    }

    // A role reads the exact property named in the table, and reaches it through the tokens
    // the block above wrote. `ui` does not inherit at all and `syntax` inherits downwards,
    // so an ancestor of the named key is not a source for it.
    for (role, property, palette_keys, required) in ROLES {
        let stated = property
            .map(|property| TokenPath::from_segments(property.split('.').chain(["default"])))
            .filter(|path| tokens.contains_key(path))
            .map(|path| reference(path.as_str()));
        let value = stated
            .or_else(|| {
                palette_keys
                    .iter()
                    .find_map(|key| source(palette, &mirror(key, variant)))
                    .map(|key| reference(&format!("colors.{key}")))
            })
            .unwrap_or_else(|| reference(&format!("colors.{}", mirror(required, variant))));
        tokens.insert(TokenPath::from_segments(["role", role]), value);
    }

    if let Some(author) = yaml::optional(scheme, "author")? {
        tokens.insert(
            TokenPath::from_segments(["meta", "author"]),
            author.to_owned(),
        );
    }

    Ok(Converted {
        name,
        variant,
        tokens,
    })
}

/// The display name the scheme goes by.
///
/// `builder.md#name-and-slug-handling` resolves an absent `name` to `slug`, and an absent
/// `slug` to `family` joined to `style` by a space. The slug is not turned back into a
/// display name: unslugifying guesses at capitalisation the file did not state.
///
/// # Errors
///
/// Returns [`Problem::Missing`] when the scheme writes none of the three, which the
/// specification requires at least one of.
fn name(scheme: &Yaml<'_>) -> Result<String, Problem> {
    if let Some(name) = yaml::optional(scheme, "name")? {
        return Ok(name.to_owned());
    }
    if let Some(slug) = yaml::optional(scheme, "slug")? {
        return Ok(slug.to_owned());
    }
    let family = yaml::optional(scheme, "family")?.ok_or(Problem::Missing("scheme.name"))?;
    Ok(match yaml::optional(scheme, "style")? {
        Some(style) => format!("{family} {style}"),
        None => family.to_owned(),
    })
}

/// The background the scheme is written for.
///
/// # Errors
///
/// Returns [`Problem::Missing`] when the scheme omits `variant` and [`Problem::Variant`]
/// when it writes one that is neither `dark` nor `light`.
fn variant(document: &Yaml<'_>) -> Result<Variant, Problem> {
    let written = yaml::required(document, "variant")?;
    Variant::parse(written).ok_or_else(|| Problem::Variant(written.to_owned()))
}

/// Reads the theming properties under `mapping` into `tokens`, `path` naming where it is.
///
/// A key may be dotted, a value may be a mapping, and the two spell the same thing, so both
/// are walked into the same path.
///
/// # Errors
///
/// Returns [`ConvertError::Segment`] for a key that is not a token segment,
/// [`ConvertError::Reserved`] for one that uses the `default` segment this encoding spends,
/// [`ConvertError::Duplicate`] when the scheme reaches one property by both spellings,
/// [`Problem::Type`] for a value that is neither a mapping nor a string, and
/// [`ConvertError::Hex`] for a colour that is not `#rrggbb`.
fn properties(
    mapping: &Yaml<'_>,
    path: &mut Vec<String>,
    tokens: &mut BTreeMap<TokenPath, String>,
) -> Result<(), ConvertError> {
    let at = path.join(".");
    for (key, value) in yaml::entries(mapping, &at)? {
        let depth = path.len();
        for segment in key.split('.') {
            if !is_segment(segment) {
                return Err(ConvertError::Segment {
                    key: format!("{at}.{key}"),
                    segment: segment.to_owned(),
                });
            }
            path.push(segment.to_owned());
            if segment == DEFAULT {
                return Err(ConvertError::Reserved {
                    key: path.join("."),
                    segment: DEFAULT,
                });
            }
        }
        if value.is_mapping() {
            properties(value, path, tokens)?;
        } else {
            let at = path.join(".");
            let written = value.as_str().ok_or_else(|| Problem::Type {
                key: at.clone(),
                expected: "a string",
            })?;
            let token = TokenPath::from_segments(path.iter().map(String::as_str).chain([DEFAULT]));
            if tokens.contains_key(&token) {
                return Err(ConvertError::Duplicate { path: token });
            }
            tokens.insert(token, literal(hex(&at, written)?));
        }
        path.truncate(depth);
    }
    Ok(())
}

/// `name` when the scheme writes it, and `None` otherwise.
///
/// The lookup is exact and does not walk to the unsuffixed key on its own. Every caller
/// states what a key falls back to, because for a terminal slot that is the hue behind the
/// variant and for a surface role it is the next surface step, and a walk built into the
/// lookup would impose the first answer on both.
fn source(palette: &Yaml<'_>, name: &str) -> Option<String> {
    yaml::writes(palette, name).then(|| name.to_owned())
}

/// `name` as a light scheme spells it, which for a dark scheme is `name` itself.
///
/// The nine-step luminance scale mirrors by index, so `black-bright` becomes `white-dim`
/// and `gray` stays where it is. A hue is not on the scale and does not move.
fn mirror(name: &str, variant: Variant) -> String {
    if variant == Variant::Dark {
        return name.to_owned();
    }
    SCALE
        .iter()
        .position(|step| *step == name)
        .and_then(|index| SCALE.get(SCALE.len() - 1 - index))
        .map_or_else(|| name.to_owned(), |step| (*step).to_owned())
}

/// `path` as the whole-string reference a theme file points at another token with.
fn reference(path: &str) -> String {
    format!("{{{{{path}}}}}")
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::Theme;
    use crate::vocabulary;
    use std::fmt::Write as _;
    use std::path::Path;

    /// A tinted8 scheme carrying the eight required colours and nothing else.
    fn minimal() -> String {
        scheme("variant: \"dark\"\n", "")
    }

    /// A scheme whose header is `header` and whose palette carries `extra` beyond the eight.
    fn scheme(header: &str, extra: &str) -> String {
        let mut text = String::from(
            "scheme:\n  system: \"tinted8\"\n  supports:\n    styling-spec: \"0.2.0\"\n  name: \"Test\"\n",
        );
        text.push_str(header);
        text.push_str("palette:\n");
        for (index, colour) in REQUIRED.into_iter().enumerate() {
            let _ = writeln!(text, "  {colour}: \"#0000{index:02x}\"");
        }
        text.push_str(extra);
        text
    }

    fn convert(text: &str) -> Result<Converted, ConvertError> {
        super::super::convert(System::Tinted8, text.as_bytes())
    }

    fn token(text: &str, path: &str) -> String {
        convert(text)
            .unwrap()
            .tokens
            .get(&TokenPath::parse(path).unwrap())
            .unwrap_or_else(|| panic!("{path} is not defined"))
            .clone()
    }

    /// The converted scheme, emitted and loaded back, which is what resolves the references.
    fn loaded(text: &str) -> Theme {
        let converted = convert(text).unwrap();
        let file =
            crate::init::theme(converted.name(), converted.variant(), converted.tokens()).unwrap();
        Theme::parse(Path::new("themes/test.toml"), &file).unwrap()
    }

    #[test]
    fn fills_every_core_token_from_a_minimal_scheme() {
        assert_eq!(vocabulary::missing(loaded(&minimal()).tokens()), Vec::new());
    }

    #[test]
    fn takes_the_eight_required_colours_into_colors() {
        assert_eq!(token(&minimal(), "colors.black"), "#000000");
        assert_eq!(token(&minimal(), "colors.white"), "#000007");
    }

    #[test]
    fn takes_an_optional_palette_variant_into_colors_under_its_own_key() {
        let text = scheme("variant: \"dark\"\n", "  red-bright: \"#fb4934\"\n");
        assert_eq!(token(&text, "colors.red-bright"), "#fb4934");
    }

    #[test]
    fn stores_a_palette_entry_in_lowercase() {
        let text = scheme("variant: \"dark\"\n", "  gray: \"#616E88\"\n");
        assert_eq!(token(&text, "colors.gray"), "#616e88");
    }

    #[test]
    fn takes_the_bright_half_of_the_terminal_from_the_bright_palette_variants() {
        let text = scheme("variant: \"dark\"\n", "  red-bright: \"#fb4934\"\n");
        assert_eq!(token(&text, "ansi.9"), "{{colors.red-bright}}");
    }

    #[test]
    fn falls_a_missing_bright_variant_back_to_the_hue_it_brightens() {
        assert_eq!(token(&minimal(), "ansi.9"), "{{colors.red}}");
    }

    #[test]
    fn keeps_the_terminal_slots_in_palette_order_whatever_the_variant() {
        let text = scheme("variant: \"light\"\n", "");
        assert_eq!(token(&text, "ansi.0"), "{{colors.black}}");
        assert_eq!(token(&text, "ansi.7"), "{{colors.white}}");
    }

    #[test]
    fn reads_a_dotted_theming_property_key() {
        let text = format!("{}syntax:\n  entity.name: \"#98971a\"\n", minimal());
        assert_eq!(token(&text, "syntax.entity.name.default"), "#98971a");
    }

    #[test]
    fn reads_a_nested_theming_property_mapping() {
        let text = format!("{}ui:\n  gutter:\n    foreground: \"#434c5e\"\n", minimal());
        assert_eq!(token(&text, "ui.gutter.foreground.default"), "#434c5e");
    }

    #[test]
    fn keeps_a_theming_property_that_is_also_the_parent_of_another() {
        let text = format!(
            "{}syntax:\n  constant: \"#88c0d0\"\n  constant.numeric: \"#b48ead\"\n",
            minimal()
        );
        assert_eq!(token(&text, "syntax.constant.default"), "#88c0d0");
        assert_eq!(token(&text, "syntax.constant.numeric.default"), "#b48ead");
    }

    #[test]
    fn writes_a_theming_property_that_is_also_a_parent_as_a_loadable_theme() {
        let text = format!(
            "{}syntax:\n  constant: \"#88c0d0\"\n  constant.numeric: \"#b48ead\"\n",
            minimal()
        );
        let theme = loaded(&text);
        let path = TokenPath::parse("syntax.constant.default").unwrap();
        assert_eq!(theme.tokens().get(&path), Some("#88c0d0"));
    }

    #[test]
    fn reports_one_theming_property_written_by_both_spellings() {
        let text = format!(
            "{}ui:\n  border.normal: \"#3b4252\"\n  border:\n    normal: \"#434c5e\"\n",
            minimal()
        );
        assert!(matches!(
            convert(&text),
            Err(ConvertError::Duplicate { ref path }) if path.as_str() == "ui.border.normal.default"
        ));
    }

    #[test]
    fn reports_a_theming_property_key_that_is_not_a_token_segment() {
        let text = format!("{}ui:\n  Accent: \"#88c0d0\"\n", minimal());
        assert!(matches!(
            convert(&text),
            Err(ConvertError::Segment { ref segment, .. }) if segment == "Accent"
        ));
    }

    #[test]
    fn reports_a_palette_key_that_is_not_a_token_segment() {
        let text = scheme("variant: \"dark\"\n", "  Gray: \"#616e88\"\n");
        assert!(matches!(
            convert(&text),
            Err(ConvertError::Segment { ref segment, .. }) if segment == "Gray"
        ));
    }

    #[test]
    fn takes_a_role_from_the_theming_property_the_scheme_states() {
        let text = format!("{}syntax:\n  keyword: \"#d79921\"\n", minimal());
        assert_eq!(token(&text, "role.keyword"), "{{syntax.keyword.default}}");
    }

    #[test]
    fn falls_a_role_back_to_the_palette_when_the_scheme_states_no_property() {
        assert_eq!(token(&minimal(), "role.keyword"), "{{colors.magenta}}");
    }

    #[test]
    fn prefers_an_optional_palette_key_to_the_required_colour_behind_it() {
        let text = scheme("variant: \"dark\"\n", "  gray-dim: \"#928374\"\n");
        assert_eq!(token(&text, "role.comment"), "{{colors.gray-dim}}");
    }

    #[test]
    fn falls_a_dimmed_role_back_to_the_hue_the_builder_would_derive_it_from() {
        assert_eq!(token(&minimal(), "role.comment"), "{{colors.white}}");
    }

    #[test]
    fn falls_a_muted_foreground_from_the_dim_grey_to_the_grey() {
        let text = scheme("variant: \"dark\"\n", "  gray: \"#928374\"\n");
        assert_eq!(token(&text, "role.comment"), "{{colors.gray}}");
        assert_eq!(token(&text, "role.inactive"), "{{colors.gray}}");
    }

    /// A fill walks down the surfaces the palette carries rather than out to `gray`, which
    /// sits halfway to the foreground.
    #[test]
    fn walks_a_fill_past_the_grey_to_the_next_surface_step() {
        let text = scheme(
            "variant: \"dark\"\n",
            "  gray: \"#928374\"\n  black-dim: \"#181825\"\n",
        );
        assert_eq!(token(&text, "role.hover-bg"), "{{colors.black-dim}}");
        assert_eq!(token(&text, "role.selection-bg"), "{{colors.black-dim}}");
    }

    /// `border` is a line drawn over the background rather than a fill, and the
    /// specification puts its default in the grey family, so it does not follow the fills
    /// down to the surfaces.
    #[test]
    fn keeps_the_border_in_the_grey_family_the_specification_defaults_it_to() {
        let text = scheme(
            "variant: \"dark\"\n",
            "  gray: \"#928374\"\n  black-dim: \"#181825\"\n",
        );
        assert_eq!(token(&text, "role.border"), "{{colors.gray}}");
    }

    /// A fill takes a grey before it takes the colour of the background it is drawn on.
    #[test]
    fn takes_a_grey_for_a_fill_rather_than_the_background_it_sits_on() {
        let text = scheme("variant: \"dark\"\n", "  gray: \"#928374\"\n");
        assert_eq!(token(&text, "role.hover-bg"), "{{colors.gray}}");
        assert_eq!(token(&text, "role.selection-bg"), "{{colors.gray}}");
        assert_eq!(token(&text, "role.bg"), "{{colors.black}}");
    }

    #[test]
    fn prefers_the_raised_surface_to_the_recessed_one_for_a_selection() {
        let text = scheme(
            "variant: \"dark\"\n",
            "  black-bright: \"#3c3836\"\n  black-dim: \"#181825\"\n",
        );
        assert_eq!(token(&text, "role.selection-bg"), "{{colors.black-bright}}");
    }

    #[test]
    fn takes_a_surface_from_the_dim_grey_when_the_scheme_writes_one() {
        let text = scheme(
            "variant: \"dark\"\n",
            "  gray-dim: \"#928374\"\n  black-dim: \"#181825\"\n",
        );
        assert_eq!(token(&text, "role.hover-bg"), "{{colors.gray-dim}}");
    }

    #[test]
    fn falls_a_surface_role_back_to_the_background_of_a_scheme_with_one_surface() {
        assert_eq!(token(&minimal(), "role.selection-bg"), "{{colors.black}}");
        assert_eq!(token(&minimal(), "role.hover-bg"), "{{colors.black}}");
    }

    #[test]
    fn reserves_the_default_leaf_a_theming_property_is_written_under() {
        let text = format!(
            "{}syntax:\n  constant: \"#111111\"\n  constant.default: \"#222222\"\n",
            minimal()
        );
        assert!(matches!(
            convert(&text),
            Err(ConvertError::Reserved { ref key, segment }) if key == "syntax.constant.default" && segment == "default"
        ));
    }

    #[test]
    fn reserves_the_default_leaf_however_the_scheme_nests_it() {
        let text = format!("{}ui:\n  selection:\n    default: \"#222222\"\n", minimal());
        assert!(matches!(
            convert(&text),
            Err(ConvertError::Reserved { ref key, .. }) if key == "ui.selection.default"
        ));
    }

    #[test]
    fn takes_the_background_from_the_dark_anchor_of_a_dark_scheme() {
        assert_eq!(token(&minimal(), "role.bg"), "{{colors.black}}");
        assert_eq!(token(&minimal(), "role.fg"), "{{colors.white}}");
    }

    #[test]
    fn mirrors_the_anchors_in_a_light_scheme() {
        let text = scheme("variant: \"light\"\n", "");
        assert_eq!(token(&text, "role.bg"), "{{colors.white}}");
        assert_eq!(token(&text, "role.fg"), "{{colors.black}}");
    }

    #[test]
    fn mirrors_a_palette_key_along_the_luminance_scale_in_a_light_scheme() {
        let text = scheme(
            "variant: \"light\"\n",
            "  black-bright: \"#6c6f85\"\n  white-dim: \"#dce0e8\"\n",
        );
        assert_eq!(token(&text, "role.linenr"), "{{colors.black-bright}}");
        assert_eq!(token(&text, "role.selection-bg"), "{{colors.white-dim}}");
    }

    #[test]
    fn leaves_a_hue_where_it_is_in_a_light_scheme() {
        let text = scheme("variant: \"light\"\n", "");
        assert_eq!(token(&text, "role.accent"), "{{colors.cyan}}");
    }

    #[test]
    fn takes_the_display_name_the_scheme_writes() {
        assert_eq!(convert(&minimal()).unwrap().name(), "Test");
    }

    #[test]
    fn builds_a_display_name_from_the_family_and_the_style() {
        let text = minimal().replace(
            "  name: \"Test\"\n",
            "  family: \"Catppuccin\"\n  style: \"Latte\"\n",
        );
        assert_eq!(convert(&text).unwrap().name(), "Catppuccin Latte");
    }

    #[test]
    fn builds_a_display_name_from_the_family_alone() {
        let text = minimal().replace("  name: \"Test\"\n", "  family: \"Gruvbox\"\n");
        assert_eq!(convert(&text).unwrap().name(), "Gruvbox");
    }

    #[test]
    fn takes_the_slug_as_a_display_name_when_the_scheme_writes_no_other() {
        let text = minimal().replace("  name: \"Test\"\n", "  slug: \"nord\"\n");
        assert_eq!(convert(&text).unwrap().name(), "nord");
    }

    #[test]
    fn reports_a_scheme_that_names_itself_in_none_of_the_three_ways() {
        let text = minimal().replace("  name: \"Test\"\n", "");
        assert!(matches!(
            convert(&text),
            Err(ConvertError::Invalid {
                source: Problem::Missing("scheme.name")
            })
        ));
    }

    #[test]
    fn takes_the_author_across_as_provenance() {
        let text = minimal().replace("  name: \"Test\"\n", "  name: \"Test\"\n  author: \"me\"\n");
        assert_eq!(token(&text, "meta.author"), "me");
    }

    #[test]
    fn takes_the_variant_from_the_scheme_metadata() {
        let text = scheme("variant: \"light\"\n", "");
        assert_eq!(convert(&text).unwrap().variant(), Variant::Light);
    }

    #[test]
    fn reports_a_scheme_that_does_not_state_a_variant() {
        let text = scheme("", "");
        assert!(matches!(
            convert(&text),
            Err(ConvertError::Invalid {
                source: Problem::Missing("variant")
            })
        ));
    }

    #[test]
    fn reports_a_variant_that_is_neither_dark_nor_light() {
        let text = scheme("variant: \"pale\"\n", "");
        assert!(matches!(
            convert(&text),
            Err(ConvertError::Invalid {
                source: Problem::Variant(_)
            })
        ));
    }

    #[test]
    fn reports_a_scheme_missing_a_required_palette_colour() {
        let text = minimal().replace("  magenta: \"#000005\"\n", "");
        assert!(matches!(
            convert(&text),
            Err(ConvertError::Slot { ref slot, .. }) if slot == "magenta"
        ));
    }

    #[test]
    fn reports_a_scheme_without_a_palette() {
        let text = "scheme:\n  system: \"tinted8\"\n  name: \"Test\"\nvariant: \"dark\"\n";
        assert!(matches!(
            convert(text),
            Err(ConvertError::Invalid {
                source: Problem::Missing("palette")
            })
        ));
    }

    #[test]
    fn reports_a_palette_entry_that_is_not_a_hex_colour() {
        let text = minimal().replace("\"#000000\"", "\"NONE\"");
        assert!(matches!(
            convert(&text),
            Err(ConvertError::Hex { ref slot, .. }) if slot == "palette.black"
        ));
    }

    #[test]
    fn reports_a_theming_property_that_is_not_a_hex_colour() {
        let text = format!("{}syntax:\n  keyword: \"NONE\"\n", minimal());
        assert!(matches!(
            convert(&text),
            Err(ConvertError::Hex { ref slot, .. }) if slot == "syntax.keyword"
        ));
    }

    #[test]
    fn mirrors_the_ends_of_the_luminance_scale_onto_each_other() {
        assert_eq!(mirror("black-dim", Variant::Light), "white-bright");
        assert_eq!(mirror("white-bright", Variant::Light), "black-dim");
        assert_eq!(mirror("gray", Variant::Light), "gray");
    }

    #[test]
    fn leaves_the_luminance_scale_alone_in_a_dark_scheme() {
        assert_eq!(mirror("black-dim", Variant::Dark), "black-dim");
    }
}
