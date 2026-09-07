//! The two names `init` proposes: what to call the target, and what to call the theme.

use std::path::Path;

use crate::config::TargetName;
use crate::theme::ThemeId;

/// A target name for the file `output`, or `None` when nothing usable can be read off it.
///
/// The component after `.config` is the tool, which is where `~/.config/nvim/lua/palette.lua`
/// gets `nvim` from. A path with no `.config` in it falls back to the file's own name. It is
/// a suggestion: the dialogue offers it and the user overrides it by typing.
#[must_use]
pub fn suggest(output: &Path) -> Option<TargetName> {
    let components: Vec<&str> = output
        .components()
        .filter_map(|component| component.as_os_str().to_str())
        .collect();
    let named = components
        .iter()
        .position(|component| *component == ".config")
        .and_then(|at| components.get(at + 1))
        .map_or_else(
            || output.file_stem().and_then(std::ffi::OsStr::to_str),
            |component| Some(*component),
        )?;
    let stem = named.split_once('.').map_or(named, |(stem, _)| stem);
    TargetName::parse(&segment(stem))
}

/// A display name for the theme `id`: hyphens become spaces and each word is capitalised.
///
/// `docs/init.md` decides that it is derived rather than asked for. It is a display string
/// nothing resolves a theme by, and `papercolor-light` cannot yield `PaperColor` however it
/// is derived.
#[must_use]
pub fn display_name(id: &ThemeId) -> String {
    id.as_str()
        .split('-')
        .map(|word| {
            let mut characters = word.chars();
            characters.next().map_or_else(String::new, |first| {
                first.to_uppercase().chain(characters).collect()
            })
        })
        .collect::<Vec<_>>()
        .join(" ")
}

/// `text` as a segment: lowercase, with everything else run together into single hyphens.
fn segment(text: &str) -> String {
    let mut segment = String::with_capacity(text.len());
    for character in text.chars() {
        if character.is_ascii_alphanumeric() {
            segment.push(character.to_ascii_lowercase());
        } else if !segment.ends_with('-') {
            segment.push('-');
        }
    }
    segment.trim_matches('-').to_owned()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn suggested(output: &str) -> Option<String> {
        suggest(Path::new(output)).map(|name| name.as_str().to_owned())
    }

    fn display(id: &str) -> String {
        display_name(&ThemeId::parse(id).unwrap())
    }

    #[test]
    fn names_a_target_after_the_directory_under_dot_config() {
        assert_eq!(
            suggested("/home/ada/.config/nvim/lua/palette.lua"),
            Some("nvim".to_owned())
        );
    }

    #[test]
    fn names_a_target_after_a_file_sitting_in_dot_config() {
        assert_eq!(
            suggested("/home/ada/.config/starship.toml"),
            Some("starship".to_owned())
        );
    }

    #[test]
    fn names_a_target_after_the_tool_rather_than_the_file() {
        assert_eq!(
            suggested("/home/ada/.config/btop/themes/coloris.theme"),
            Some("btop".to_owned())
        );
    }

    #[test]
    fn falls_back_to_the_file_when_there_is_no_dot_config() {
        assert_eq!(suggested("/srv/palette.lua"), Some("palette".to_owned()));
    }

    #[test]
    fn runs_what_is_not_a_segment_together_into_hyphens() {
        assert_eq!(
            suggested("/srv/PaperColor Light.conf"),
            Some("papercolor-light".to_owned())
        );
    }

    #[test]
    fn suggests_nothing_for_a_name_with_no_segment_in_it() {
        assert_eq!(suggested("/srv/---"), None);
    }

    #[test]
    fn capitalises_each_word_of_a_display_name() {
        assert_eq!(display("papercolor-light"), "Papercolor Light");
    }

    #[test]
    fn leaves_a_one_word_display_name_as_one_word() {
        assert_eq!(display("nord"), "Nord");
    }

    #[test]
    fn keeps_a_digit_in_a_display_name() {
        assert_eq!(display("base16-ocean"), "Base16 Ocean");
    }
}
