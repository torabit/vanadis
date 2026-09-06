//! Unified diffs, for `apply --diff`.
//!
//! Both sides name the same file, because they are the same file: what it holds now and what
//! the apply would put there. There is no `a/` and `b/` prefix to strip, and an output path
//! is absolute, so a prefix would only produce a doubled slash.

use std::path::Path;

use similar::TextDiff;

/// A unified diff of `before` against `after`, both labelled `path`.
///
/// Empty when the two are identical, so a caller can print the result unconditionally and
/// say nothing about a target that would not change.
#[must_use]
pub fn unified(path: &Path, before: &str, after: &str) -> String {
    if before == after {
        return String::new();
    }

    let label = path.display().to_string();
    TextDiff::from_lines(before, after)
        .unified_diff()
        .header(&label, &label)
        .to_string()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn diff(before: &str, after: &str) -> String {
        unified(Path::new("out/one.conf"), before, after)
    }

    #[test]
    fn says_nothing_about_two_identical_texts() {
        assert!(diff("bg=#eeeeee\n", "bg=#eeeeee\n").is_empty());
    }

    #[test]
    fn labels_both_sides_with_the_file_they_describe() {
        let diff = diff("bg=#eeeeee\n", "bg=#2e3440\n");
        assert!(
            diff.starts_with("--- out/one.conf\n+++ out/one.conf\n"),
            "{diff}"
        );
    }

    #[test]
    fn marks_the_line_that_changed() {
        let diff = diff("bg=#eeeeee\n", "bg=#2e3440\n");
        assert!(diff.contains("-bg=#eeeeee\n"), "{diff}");
        assert!(diff.contains("+bg=#2e3440\n"), "{diff}");
    }

    #[test]
    fn shows_a_whole_file_as_added_when_there_was_none() {
        let diff = diff("", "bg=#2e3440\n");
        assert!(diff.contains("+bg=#2e3440\n"), "{diff}");
        assert!(!diff.contains("-bg"), "{diff}");
    }
}
