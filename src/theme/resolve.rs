//! The reference graph: what is wrong with it, and what it resolves to.

use std::collections::BTreeMap;

use super::Problem;
use super::walk::{Definition, RawValue};
use crate::token::{TokenPath, Tokens};

/// Every problem in the reference graph, in path order.
pub(super) fn problems(
    definitions: &BTreeMap<TokenPath, Definition>,
    namespaces: &BTreeMap<TokenPath, usize>,
) -> Vec<Problem> {
    let mut problems = Vec::new();
    let mut edges: BTreeMap<&TokenPath, &TokenPath> = BTreeMap::new();

    for (path, definition) in definitions {
        let RawValue::Reference(target) = &definition.value else {
            continue;
        };
        if definitions.contains_key(target) {
            edges.insert(path, target);
        } else if namespaces.contains_key(target) {
            problems.push(Problem::Namespace {
                path: path.clone(),
                line: definition.line,
                target: target.clone(),
            });
        } else {
            problems.push(Problem::Undefined {
                path: path.clone(),
                line: definition.line,
                target: target.clone(),
            });
        }
    }

    problems.extend(cycles(definitions, &edges));
    problems
}

/// Where a token sits in the walk below.
#[derive(Clone, Copy, PartialEq, Eq)]
enum State {
    /// On the path currently being followed.
    OnPath,
    /// Walked already, and known not to reach a cycle it is part of.
    Settled,
}

/// Every cycle, each reported once.
///
/// A theme value is either a literal or exactly one reference, so a token has at most one
/// outgoing edge. Following that edge from each token and stopping when the walk meets its
/// own path finds each cycle exactly once, which is what strongly connected components would
/// be needed for in a graph where a node can branch.
fn cycles(
    definitions: &BTreeMap<TokenPath, Definition>,
    edges: &BTreeMap<&TokenPath, &TokenPath>,
) -> Vec<Problem> {
    let mut state: BTreeMap<&TokenPath, State> = BTreeMap::new();
    let mut cycles = Vec::new();

    for start in definitions.keys() {
        let mut walked: Vec<&TokenPath> = Vec::new();
        let mut current = start;

        loop {
            match state.get(current) {
                Some(State::Settled) => break,
                Some(State::OnPath) => {
                    if let Some(at) = walked.iter().position(|token| *token == current) {
                        cycles.push(from_line(&walked[at..], definitions));
                    }
                    break;
                }
                None => {
                    state.insert(current, State::OnPath);
                    walked.push(current);
                    match edges.get(current) {
                        Some(next) => current = next,
                        None => break,
                    }
                }
            }
        }

        for token in walked {
            state.insert(token, State::Settled);
        }
    }

    cycles
}

/// The cycle written from the member the file states first, so the line and the first token
/// named in the message are the same place.
fn from_line(members: &[&TokenPath], definitions: &BTreeMap<TokenPath, Definition>) -> Problem {
    let line_of = |token: &TokenPath| definitions.get(token).map_or(0, |member| member.line);
    let first = members
        .iter()
        .enumerate()
        .min_by_key(|(_, token)| line_of(token))
        .map_or(0, |(at, _)| at);

    Problem::Cycle {
        members: members[first..]
            .iter()
            .chain(&members[..first])
            .map(|token| (*token).clone())
            .collect(),
        line: members.get(first).map_or(0, |token| line_of(token)),
    }
}

/// Follows every reference down to the literal it names.
///
/// Callers resolve only a graph [`problems`] found nothing wrong with, so every chain ends in
/// a literal.
pub(super) fn resolve(definitions: &BTreeMap<TokenPath, Definition>) -> Tokens {
    let mut resolved: BTreeMap<TokenPath, String> = BTreeMap::new();

    for (path, definition) in definitions {
        let mut chain = vec![path.clone()];
        let mut current = definition;

        let value = loop {
            match &current.value {
                RawValue::Literal(text) => break text.clone(),
                RawValue::Reference(target) => {
                    if let Some(value) = resolved.get(target) {
                        break value.clone();
                    }
                    let Some(next) = definitions.get(target) else {
                        break String::new();
                    };
                    chain.push(target.clone());
                    current = next;
                }
            }
        };

        for member in chain {
            resolved.insert(member, value.clone());
        }
    }

    resolved.into_iter().collect()
}

#[cfg(test)]
mod tests {
    use std::collections::BTreeSet;

    use proptest::prelude::*;

    use super::*;

    /// Nodes `role.t0`..`role.tN`, each either a literal or a reference to another node.
    ///
    /// A theme value is one whole reference, so this is every reference graph a theme can
    /// have: out-degree at most one.
    fn graph(edges: &[Option<usize>]) -> BTreeMap<TokenPath, Definition> {
        edges
            .iter()
            .enumerate()
            .map(|(node, edge)| {
                let value = match edge {
                    Some(target) => RawValue::Reference(node_path(target % edges.len())),
                    None => RawValue::Literal(format!("#00000{node:x}")),
                };
                (
                    node_path(node),
                    Definition {
                        value,
                        line: node + 1,
                    },
                )
            })
            .collect()
    }

    fn node_path(node: usize) -> TokenPath {
        TokenPath::from_segments(["role", &format!("t{node}")])
    }

    /// Whether the graph has a cycle, worked out by following each node to its end.
    fn has_cycle(edges: &[Option<usize>]) -> bool {
        (0..edges.len()).any(|start| {
            let mut seen = BTreeSet::new();
            let mut current = start;
            loop {
                if !seen.insert(current) {
                    return true;
                }
                match edges[current] {
                    Some(target) => current = target % edges.len(),
                    None => return false,
                }
            }
        })
    }

    fn reported_cycles(problems: &[Problem]) -> Vec<&Vec<TokenPath>> {
        problems
            .iter()
            .filter_map(|problem| match problem {
                Problem::Cycle { members, .. } => Some(members),
                _ => None,
            })
            .collect()
    }

    proptest! {
        #[test]
        fn reports_a_cycle_exactly_when_the_graph_has_one(
            edges in prop::collection::vec(prop::option::of(0usize..8), 1..8)
        ) {
            let definitions = graph(&edges);
            let problems = problems(&definitions, &BTreeMap::new());
            prop_assert_eq!(!reported_cycles(&problems).is_empty(), has_cycle(&edges));
        }

        #[test]
        fn reports_each_token_in_at_most_one_cycle(
            edges in prop::collection::vec(prop::option::of(0usize..8), 1..8)
        ) {
            let definitions = graph(&edges);
            let problems = problems(&definitions, &BTreeMap::new());
            let members: Vec<&TokenPath> = reported_cycles(&problems)
                .into_iter()
                .flatten()
                .collect();
            let distinct: BTreeSet<&&TokenPath> = members.iter().collect();
            prop_assert_eq!(members.len(), distinct.len());
        }

        #[test]
        fn resolves_every_token_of_an_acyclic_graph(
            edges in prop::collection::vec(prop::option::of(0usize..8), 1..8)
        ) {
            prop_assume!(!has_cycle(&edges));
            let definitions = graph(&edges);
            let tokens = resolve(&definitions);
            for node in 0..edges.len() {
                let mut current = node;
                while let Some(target) = edges[current] {
                    current = target % edges.len();
                }
                let terminal = format!("#00000{current:x}");
                prop_assert_eq!(tokens.get(&node_path(node)), Some(terminal.as_str()));
            }
        }
    }
}
