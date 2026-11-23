//! Bounded, goal-free reachable graphs of confirmed operations.
//!
//! Nodes are whole-model fact and monitor configurations. Derived predicates are
//! annotations, not extra nodes. Exploration neither mutates its basis nor invokes
//! actions or delivers events to reactions.

mod types;

use std::collections::HashMap;

use crate::{
    Diagnostic, Machine,
    finite::{Domain, identity, validation},
    runtime::RuntimeState,
};
pub use types::*;

/// Classify the selected operation domain without traversing it.
pub fn analyze(
    machine: &Machine,
    state: &RuntimeState,
    operators: Option<Vec<Operator>>,
) -> Result<Support, Diagnostic> {
    Ok(prepare(machine, state, operators)?.1)
}

fn prepare<'a>(
    machine: &'a Machine,
    state: &RuntimeState,
    operators: Option<Vec<Operator>>,
) -> Result<(Domain<'a>, Support), Diagnostic> {
    state.check_machine(machine)?;
    let (domain, mut reasons) =
        Domain::prepare(machine, operators.unwrap_or_else(|| Domain::local(machine)))?;
    if let Err(mut reason) = state.check_standalone() {
        reason.code = "foreign_evidence".into();
        reasons.push(reason);
    }
    reasons.extend(validation::unknowns(state));
    let excluded = Excluded {
        actions: machine
            .document()
            .branches
            .iter()
            .map(|branch| branch.actions.len())
            .sum(),
        reactions: machine
            .programs
            .iter()
            .map(|program| program.reactions.len())
            .sum(),
    };
    if domain.operators.is_empty() && (excluded.actions > 0 || excluded.reactions > 0) {
        reasons.push(
            Diagnostic::new(
                "no_confirmed_operations",
                "This model has external behavior but no selected confirmed operations",
                None,
            )
            .at_path("/branches"),
        );
    }
    let support = Support {
        supported: reasons.is_empty(),
        scope: "confirmed_operations",
        operators: domain.operators.clone(),
        excluded,
        reasons,
    };
    Ok((domain, support))
}

/// Breadth-first traversal, retaining every applicable edge between kept nodes.
/// A boundary entry records an applicable transition whose target exceeds the cap.
pub fn explore(
    machine: &Machine,
    state: &RuntimeState,
    options: Options,
) -> Result<Graph, Diagnostic> {
    if options.max_states == 0 {
        return Err(
            Diagnostic::new("invalid_planning", "max_states must be positive", None)
                .at_path("/max_states"),
        );
    }
    let (domain, support) = prepare(machine, state, options.operators)?;
    let mut graph = Graph {
        status: if support.supported {
            Status::Complete
        } else {
            Status::Unsupported
        },
        support,
        max_states: options.max_states,
        basis: Basis {
            revision: state.revision(),
            context: state.context().clone(),
            generation: state.generation(),
            step: state.step(),
        },
        initial: None,
        nodes: Vec::new(),
        edges: Vec::new(),
        boundary: Vec::new(),
    };
    if !graph.support.supported {
        return Ok(graph);
    }
    let mut states = vec![state.clone()];
    let mut identities = HashMap::from([(identity(state)?, 0)]);
    graph.initial = Some("S0".into());
    graph.nodes.push(node(machine, state, 0)?);
    let mut cursor = 0;
    while cursor < states.len() {
        for operator in &domain.operators {
            let Some(next) = domain.apply(&states[cursor], operator)? else {
                continue;
            };
            let key = identity(&next)?;
            let target = match identities.get(&key) {
                Some(index) => *index,
                None if states.len() < options.max_states => {
                    let index = states.len();
                    graph.nodes.push(node(machine, &next, index)?);
                    identities.insert(key, index);
                    states.push(next);
                    index
                }
                None => {
                    graph.boundary.push(Boundary {
                        source: format!("S{cursor}"),
                        operator: operator.name.clone(),
                    });
                    continue;
                }
            };
            graph.edges.push(Edge {
                source: format!("S{cursor}"),
                target: format!("S{target}"),
                operator: operator.name.clone(),
            });
        }
        cursor += 1;
    }
    if !graph.boundary.is_empty() {
        graph.status = Status::StateLimit;
    }
    Ok(graph)
}

fn node(machine: &Machine, state: &RuntimeState, index: usize) -> Result<Node, Diagnostic> {
    let mut branches = std::collections::BTreeMap::new();
    for branch in &machine.document().branches {
        let snapshot = state.branch(&branch.name).expect("compiled branch");
        branches.insert(
            branch.name.clone(),
            Branch {
                facts: snapshot.facts.clone(),
                state: snapshot.state.clone(),
                memberships: snapshot.memberships.clone(),
                labels: snapshot.labels.clone(),
                activation: branch
                    .activating
                    .as_ref()
                    .map(|_| state.activation(machine, &branch.name))
                    .transpose()?,
            },
        );
    }
    Ok(Node {
        id: format!("S{index}"),
        branches,
    })
}
