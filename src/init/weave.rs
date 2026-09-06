//! Building the template: substituting the colours that were named, escaping the rest.
//!
//! `docs/init.md` decides that an apply straight after an init reproduces the original file
//! byte for byte. Everything here exists to make that true, and
//! [`crate::init::plan`] is what refuses to write when it is not.

use crate::init::scan::Occurrence;
use crate::token::TokenPath;

/// One occurrence, and the token that replaces it.
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct Binding {
    start: usize,
    end: usize,
    token: TokenPath,
}

impl Binding {
    /// Binds `occurrence` to `token`.
    #[must_use]
    pub fn new(occurrence: &Occurrence, token: TokenPath) -> Self {
        let range = occurrence.range();
        Self {
            start: range.start,
            end: range.end,
            token,
        }
    }

    /// The token it replaces its occurrence with.
    #[must_use]
    pub fn token(&self) -> &TokenPath {
        &self.token
    }
}

/// Rewrites `source` as a template, replacing every bound occurrence with its token.
///
/// Occurrences that nothing binds keep their literal, which is what a hex that is not a
/// colour needs. Everything the renderer would otherwise consume is escaped, so a source
/// with template syntax of its own survives the round trip.
#[must_use]
pub fn weave(source: &str, bindings: &[Binding]) -> String {
    let mut ordered: Vec<&Binding> = bindings.iter().collect();
    ordered.sort_by_key(|binding| binding.start);

    let mut template = String::with_capacity(source.len());
    let mut copied = 0;
    let mut next = 0;
    let mut at = 0;

    while at < source.len() {
        if let Some(binding) = ordered.get(next).filter(|binding| binding.start == at) {
            template.push_str(&source[copied..at]);
            template.push_str("{{");
            template.push_str(binding.token.as_str());
            template.push_str("}}");
            at = binding.end;
            copied = at;
            next += 1;
            continue;
        }
        if let Some((escaped, length)) = escape(&source[at..]) {
            template.push_str(&source[copied..at]);
            template.push_str(escaped);
            at += length;
            copied = at;
            continue;
        }
        at += 1;
        while at < source.len() && !source.is_char_boundary(at) {
            at += 1;
        }
    }
    template.push_str(&source[copied..]);
    template
}

/// What `rest` has to be written as for the renderer to give it back unchanged.
///
/// Every doubled brace is escaped, whether or not it opens something the renderer would
/// have acted on. Escaping only the markers it acts on is not enough: substituting a colour
/// that sits directly after a `{{` puts a second `{{` against the first, and the renderer
/// reads the four together as one escape. Doubling every pair leaves no unescaped `{{` in
/// the template for an insertion to run into.
///
/// A single brace is left alone. The renderer copies it, and rio's `{workspace}` and `{-}`
/// are what it is for.
fn escape(rest: &str) -> Option<(&'static str, usize)> {
    if rest.starts_with("{{") {
        return Some(("{{{{", 2));
    }
    if rest.starts_with("}}") {
        return Some(("}}}}", 2));
    }
    None
}

#[cfg(test)]
mod tests {
    use proptest::prelude::*;

    use super::*;
    use crate::init::scan;
    use crate::template::Template;
    use crate::token::Tokens;

    fn path(text: &str) -> TokenPath {
        TokenPath::parse(text).unwrap()
    }

    /// Binds every occurrence of every value to `role.<n>`, numbered by value.
    fn everything(source: &str) -> Vec<Binding> {
        scan::scan(source)
            .colours()
            .iter()
            .enumerate()
            .flat_map(|(index, colour)| {
                colour.occurrences().iter().map(move |occurrence| {
                    Binding::new(occurrence, path(&format!("role.c{index}")))
                })
            })
            .collect()
    }

    fn woven(source: &str) -> String {
        weave(source, &everything(source))
    }

    /// Renders `template` with whatever `source` said each token was worth.
    fn rendered(source: &str, template: &str) -> String {
        let tokens: Tokens = scan::scan(source)
            .colours()
            .iter()
            .enumerate()
            .map(|(index, colour)| (path(&format!("role.c{index}")), colour.value().to_owned()))
            .collect();
        Template::new("t.in", template).render(&tokens).unwrap()
    }

    #[test]
    fn substitutes_a_bound_occurrence() {
        assert_eq!(woven("bg = \"#eeeeee\""), "bg = \"{{role.c0}}\"");
    }

    #[test]
    fn substitutes_every_occurrence_of_one_value() {
        assert_eq!(woven("#eeeeee #eeeeee"), "{{role.c0}} {{role.c0}}");
    }

    #[test]
    fn keeps_an_occurrence_nothing_binds() {
        assert_eq!(weave("bg = \"#eeeeee\"", &[]), "bg = \"#eeeeee\"");
    }

    #[test]
    fn binds_two_occurrences_of_one_value_to_different_tokens() {
        let source = "a = \"#878787\"\nb = \"#878787\"";
        let colours = scan::scan(source);
        let occurrences = colours.colours()[0].occurrences();
        let bindings = [
            Binding::new(&occurrences[0], path("role.comment")),
            Binding::new(&occurrences[1], path("role.inactive")),
        ];
        assert_eq!(
            weave(source, &bindings),
            "a = \"{{role.comment}}\"\nb = \"{{role.inactive}}\""
        );
    }

    #[test]
    fn escapes_a_marker_the_renderer_would_substitute() {
        assert_eq!(
            woven("title = \"{{columns}}\""),
            "title = \"{{{{columns}}}}\""
        );
    }

    #[test]
    fn escapes_doubled_braces() {
        assert_eq!(woven("{{{{a}}}}"), "{{{{{{{{a}}}}}}}}");
    }

    #[test]
    fn escapes_a_marker_the_renderer_would_have_passed_through() {
        assert_eq!(
            woven("{{ title }} {{role.*}}"),
            "{{{{ title }}}} {{{{role.*}}}}"
        );
    }

    #[test]
    fn escapes_a_doubled_brace_a_substitution_would_otherwise_run_into() {
        assert_eq!(woven("{{#eeeeee"), "{{{{{{role.c0}}");
    }

    #[test]
    fn leaves_a_single_brace_alone() {
        assert_eq!(woven("{workspace} {-}"), "{workspace} {-}");
    }

    #[test]
    fn leaves_multibyte_text_intact() {
        assert_eq!(
            woven("# 配色は #eeeeee を参照"),
            "# 配色は {{role.c0}} を参照"
        );
    }

    #[test]
    fn renders_a_woven_template_back_to_the_source() {
        let source = "bg = \"#eeeeee\"\nfg = \"#444444\"\ntitle = \"{{columns}}\"\n";
        assert_eq!(rendered(source, &woven(source)), source);
    }

    #[test]
    fn renders_an_unbound_source_back_to_itself() {
        let source = "{{{{a}}}} {{ title }} {} }} {{role.*}} #eeeeee\n";
        assert_eq!(rendered(source, &weave(source, &[])), source);
    }

    proptest! {
        /// The whole guarantee, on text nobody chose: escaping is reversed by rendering.
        #[test]
        fn renders_any_escaped_source_back_to_itself(
            source in prop::collection::vec(
                prop_oneof![
                    Just("{".to_owned()),
                    Just("}".to_owned()),
                    Just("{{".to_owned()),
                    Just("}}".to_owned()),
                    Just("role.bg".to_owned()),
                    Just(" title ".to_owned()),
                    Just("配".to_owned()),
                    "[a-z .*]{0,4}",
                ],
                0..24,
            ).prop_map(|parts| parts.concat())
        ) {
            prop_assert_eq!(rendered(&source, &weave(&source, &[])), source);
        }

        /// And it still holds when colours are substituted at the same time.
        #[test]
        fn renders_any_woven_source_back_to_itself(
            source in prop::collection::vec(
                prop_oneof![
                    Just("{{".to_owned()),
                    Just("}}".to_owned()),
                    Just("#eeeeee".to_owned()),
                    Just("#444444".to_owned()),
                    Just("role.bg".to_owned()),
                    "[a-z .*]{0,4}",
                ],
                0..24,
            ).prop_map(|parts| parts.concat())
        ) {
            prop_assert_eq!(rendered(&source, &woven(&source)), source);
        }
    }
}
