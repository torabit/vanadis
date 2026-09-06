//! Reading an upstream colour scheme into the theme model.
//!
//! `docs/theme-format.md` decides what the converted theme holds and
//! `docs/core-vocabulary.md` decides the `base00`-`base0F` mapping and the rule this module
//! is held to: a converter fills the entire core or the conversion is a bug.
//!
//! The converter takes bytes. It knows nothing about where they came from, so a file on disk
//! and a scheme fetched over the network go down the same path.

use std::collections::BTreeMap;
use std::str::Utf8Error;

use saphyr::ScanError;
use thiserror::Error;

use crate::theme::Variant;
use crate::token::TokenPath;

mod base16;
mod yaml;

pub use base16::System;

/// A scheme converted to the theme model.
///
/// This is not a theme and not a file. `meta.id` is deliberately absent: the identifier is
/// the theme's filename, which the converter does not choose.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Converted {
    name: String,
    variant: Variant,
    tokens: BTreeMap<TokenPath, String>,
}

impl Converted {
    /// The upstream display name, verbatim, for `meta.name`.
    #[must_use]
    pub fn name(&self) -> &str {
        &self.name
    }

    /// The background the scheme is written for, for `meta.variant`.
    #[must_use]
    pub fn variant(&self) -> Variant {
        self.variant
    }

    /// Every token the theme file will hold, `meta.name` and `meta.variant` aside.
    #[must_use]
    pub fn tokens(&self) -> &BTreeMap<TokenPath, String> {
        &self.tokens
    }
}

/// Converting an upstream scheme failed.
#[derive(Debug, Error)]
pub enum SchemeError {
    /// The bytes are not UTF-8.
    #[error("the scheme is not UTF-8: {source}")]
    Utf8 {
        /// Where the decode failed.
        source: Utf8Error,
    },
    /// The bytes are not valid YAML.
    #[error("the scheme is not valid YAML: {source}")]
    Yaml {
        /// The parse error.
        source: ScanError,
    },
    /// The YAML holds no document at all.
    #[error("the scheme holds no YAML document")]
    Empty,
    /// The YAML holds a document, and it is not a mapping.
    #[error("the scheme is not a YAML mapping")]
    Document,
    /// A key the scheme has to carry is not written.
    #[error("the scheme does not carry `{key}`")]
    Missing {
        /// The key that is not written.
        key: String,
    },
    /// A key is written but holds the wrong kind of value.
    #[error("`{key}` is not {expected}")]
    Type {
        /// The key that holds it.
        key: String,
        /// What the key has to hold.
        expected: &'static str,
    },
    /// `system` names a scheme system this does not convert.
    #[error("`{system}` is not a scheme system vanadis converts")]
    System {
        /// The system as the scheme spells it.
        system: String,
    },
    /// `variant` is written and is neither `dark` nor `light`.
    #[error("`variant` is `{variant}`, which is neither `dark` nor `light`")]
    Variant {
        /// The variant as the scheme spells it.
        variant: String,
    },
    /// The palette is missing a slot the scheme's own system requires.
    #[error("the {system} palette does not carry `{slot}`")]
    Slot {
        /// The system the scheme declares.
        system: System,
        /// The slot that is not written.
        slot: String,
    },
    /// A palette entry is not a hex colour.
    #[error("`{slot}` is `{value}`, which is not `#rrggbb`")]
    Hex {
        /// The slot that holds it.
        slot: String,
        /// The value as the scheme writes it.
        value: String,
    },
}

/// Converts the bytes of an upstream scheme into the theme model.
///
/// The scheme's own `system` key decides how it is read. base16 and base24 are what this
/// converts today; anything else is [`SchemeError::System`].
///
/// # Errors
///
/// Returns [`SchemeError`] when the bytes are not UTF-8, are not YAML, declare a system this
/// does not convert, omit a key or a palette slot that system requires, or write a palette
/// entry that is not `#rrggbb`.
pub fn convert(bytes: &[u8]) -> Result<Converted, SchemeError> {
    let text = std::str::from_utf8(bytes).map_err(|source| SchemeError::Utf8 { source })?;
    let document = yaml::document(text)?;
    let system = System::parse(yaml::required(&document, "system")?)?;
    base16::convert(&document, system)
}

/// Which background a scheme is written for, taken from the scheme or read off `base00`.
///
/// `variant` is what the issue names first and every scheme in `tinted-theming/schemes`
/// carries it. The fallback is the luminance of the default background, using the formula
/// and the threshold `tests/fixtures/templates/herdr/host-colors.py.in` already answers the
/// same question with: sRGB coefficients on the gamma-encoded channels, light above one
/// half. Matching it keeps one rule in the repository rather than two.
///
/// # Errors
///
/// Returns [`SchemeError::Variant`] when the scheme writes a variant that is neither `dark`
/// nor `light`. An absent variant is inferred, a wrong one is a bug in the scheme.
fn variant(written: Option<&str>, background: [u8; 3]) -> Result<Variant, SchemeError> {
    match written {
        Some(text) => Variant::parse(text).ok_or_else(|| SchemeError::Variant {
            variant: text.to_owned(),
        }),
        None => Ok(if luma(background) > 0.5 {
            Variant::Light
        } else {
            Variant::Dark
        }),
    }
}

/// The sRGB luma of a colour, from its red, green and blue.
fn luma([red, green, blue]: [u8; 3]) -> f64 {
    let scaled = |channel: u8| f64::from(channel) / 255.0;
    0.2126 * scaled(red) + 0.7152 * scaled(green) + 0.0722 * scaled(blue)
}

/// `value` as red, green and blue, or [`SchemeError::Hex`].
///
/// `docs/theme-format.md` decides that a hex literal is `#` and six hex digits, stored
/// lowercase. Upstream schemes write exactly that, in either case, so the converter accepts
/// exactly that: three-digit shorthand and eight-digit `#rrggbbaa` are rejected rather than
/// expanded or truncated, neither of which the scheme asked for.
///
/// The channels are parsed here rather than re-read off the literal later, so no caller has
/// to handle a colour it has already validated failing to parse.
fn hex(slot: &str, value: &str) -> Result<[u8; 3], SchemeError> {
    let malformed = || SchemeError::Hex {
        slot: slot.to_owned(),
        value: value.to_owned(),
    };
    let digits = value.strip_prefix('#').ok_or_else(malformed)?.as_bytes();
    let [r0, r1, g0, g1, b0, b1] = <[u8; 6]>::try_from(digits).map_err(|_| malformed())?;
    let channel = |high: u8, low: u8| match (nibble(high), nibble(low)) {
        (Some(high), Some(low)) => Ok(high * 16 + low),
        _ => Err(malformed()),
    };
    Ok([channel(r0, r1)?, channel(g0, g1)?, channel(b0, b1)?])
}

/// What one hex digit is worth, in either case.
///
/// Decoded here rather than through `u8::from_str_radix`, which also accepts a leading `+`
/// and would let `#+f+f+f` through as a colour.
fn nibble(byte: u8) -> Option<u8> {
    match byte {
        b'0'..=b'9' => Some(byte - b'0'),
        b'a'..=b'f' => Some(byte - b'a' + 10),
        b'A'..=b'F' => Some(byte - b'A' + 10),
        _ => None,
    }
}

/// Red, green and blue as the hex literal a theme file holds.
fn literal([red, green, blue]: [u8; 3]) -> String {
    format!("#{red:02x}{green:02x}{blue:02x}")
}

#[cfg(test)]
mod tests {
    use super::*;

    /// `value` read and written back out, which is what a theme file ends up holding.
    fn round_trip(value: &str) -> String {
        literal(hex("base00", value).unwrap())
    }

    #[test]
    fn reads_the_three_channels_of_a_hex_literal() {
        assert_eq!(hex("base00", "#1d2021").unwrap(), [0x1d, 0x20, 0x21]);
    }

    #[test]
    fn stores_a_hex_literal_in_lowercase() {
        assert_eq!(round_trip("#1D2021"), "#1d2021");
    }

    #[test]
    fn keeps_a_hex_literal_that_is_already_lowercase() {
        assert_eq!(round_trip("#1d2021"), "#1d2021");
    }

    #[test]
    fn pads_a_channel_below_sixteen() {
        assert_eq!(literal([0x00, 0x0f, 0xff]), "#000fff");
    }

    #[test]
    fn rejects_a_hex_literal_without_a_hash() {
        assert!(hex("base00", "1d2021").is_err());
    }

    #[test]
    fn rejects_three_digit_shorthand() {
        assert!(hex("base00", "#eee").is_err());
    }

    #[test]
    fn rejects_a_hex_literal_carrying_alpha() {
        assert!(hex("base00", "#1d2021ff").is_err());
    }

    #[test]
    fn rejects_a_hex_literal_that_is_not_hex() {
        assert!(hex("base00", "#gggggg").is_err());
    }

    #[test]
    fn rejects_a_signed_channel() {
        assert!(hex("base00", "#+f+f+f").is_err());
    }

    #[test]
    fn rejects_a_seventh_digit() {
        assert!(hex("base00", "#1d20211").is_err());
    }

    #[test]
    fn names_the_slot_a_malformed_hex_literal_sits_in() {
        let error = hex("base0a", "#eee").unwrap_err();
        assert!(error.to_string().contains("base0a"), "{error}");
    }

    /// The variant inferred for a background written as a hex literal.
    fn inferred(background: &str) -> Variant {
        variant(None, hex("base00", background).unwrap()).unwrap()
    }

    #[test]
    fn takes_a_written_variant_over_the_background() {
        assert_eq!(
            variant(Some("light"), [0x00, 0x00, 0x00]).unwrap(),
            Variant::Light
        );
    }

    #[test]
    fn rejects_a_written_variant_that_is_neither_dark_nor_light() {
        assert!(variant(Some("pale"), [0x00, 0x00, 0x00]).is_err());
    }

    #[test]
    fn infers_a_dark_variant_from_a_dark_background() {
        assert_eq!(inferred("#1d2021"), Variant::Dark);
    }

    #[test]
    fn infers_a_light_variant_from_a_light_background() {
        assert_eq!(inferred("#fdf6e3"), Variant::Light);
    }

    #[test]
    fn infers_light_on_the_bright_side_of_the_luminance_boundary() {
        assert_eq!(inferred("#808080"), Variant::Light);
    }

    #[test]
    fn infers_dark_on_the_dark_side_of_the_luminance_boundary() {
        assert_eq!(inferred("#7f7f7f"), Variant::Dark);
    }

    #[test]
    fn weights_green_above_red_and_blue() {
        assert!(luma([0x00, 0xff, 0x00]) > luma([0xff, 0x00, 0x00]));
        assert!(luma([0xff, 0x00, 0x00]) > luma([0x00, 0x00, 0xff]));
    }
}
