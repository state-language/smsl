mod abstractions;
pub(crate) mod derived;
pub(crate) mod memory;

use std::collections::{HashMap, HashSet};

use crate::diagnostic::{Diagnostic, Location};
use crate::language::model::{Branch, Document, Fact, State, Value};

pub fn validate(document: &Document) -> Result<(), Diagnostic> {
    if document.version != crate::LANGUAGE_VERSION {
        return Err(Diagnostic::new(
            "unsupported_version",
            format!("Unsupported SMSL version {:?}", document.version),
            None,
        )
        .at_path("/version"));
    }
    if document.branches.is_empty() {
        return Err(Diagnostic::new(
            "invalid_assignment",
            "A document must declare at least one branch",
            None,
        ));
    }
    let mut branches = HashMap::new();
    for (index, branch) in document.branches.iter().enumerate() {
        insert_name(
            &mut branches,
            &branch.name,
            branch,
            "branch",
            branch.location,
        )
        .map_err(|error| error.at_path(format!("/branches/{index}/name")))?;
        validate_branch(branch)
            .map_err(|error| error.prepend_path(&format!("/branches/{index}")))?;
    }
    validate_hierarchy(document, &branches)?;
    crate::language::validation::memory::validate_memory(document)?;
    derived::order(document)?;
    abstractions::validate(document)
}

fn insert_name<'a, T>(
    names: &mut HashMap<&'a str, T>,
    name: &'a str,
    value: T,
    kind: &str,
    location: Option<Location>,
) -> Result<(), Diagnostic> {
    if name.is_empty() {
        return Err(Diagnostic::new(
            "invalid_assignment",
            format!("A {kind} name cannot be empty"),
            location,
        ));
    }
    if names.insert(name, value).is_some() {
        return Err(Diagnostic::new(
            "duplicate_name",
            format!("Duplicate {kind} {name:?}"),
            location,
        ));
    }
    Ok(())
}

fn validate_branch(branch: &Branch) -> Result<(), Diagnostic> {
    let mut facts = HashMap::new();
    for (index, fact) in branch.facts.iter().enumerate() {
        let path = format!("/facts/{index}");
        insert_name(&mut facts, &fact.name, fact, "fact", fact.location)
            .map_err(|error| error.at_path(format!("{path}/name")))?;
        if fact.domain.is_some() {
            return Err(Diagnostic::new(
                "unknown_reference",
                "Named domains must be resolved through module linking",
                fact.location,
            )
            .at_path(format!("{path}/domain")));
        }
        domain(&fact.values, fact.range, fact.location)
            .map_err(|error| error.prepend_path(&path))?;
    }
    let mut states = HashMap::new();
    for (index, state) in branch.states.iter().enumerate() {
        insert_name(&mut states, &state.name, state, "state", state.location)
            .map_err(|error| error.at_path(format!("/states/{index}/name")))?;
    }
    if let Some(initial) = &branch.initial {
        if !states.contains_key(initial.as_str()) {
            return Err(unknown("initial state", initial, branch.location).at_path("/initial"));
        }
    } else if !states.is_empty() {
        return Err(Diagnostic::new(
            "invalid_assignment",
            "A branch with control states requires an initial state",
            branch.location,
        )
        .at_path("/initial"));
    }
    for (index, state) in branch.states.iter().enumerate() {
        validate_state(state, &facts, &states)
            .map_err(|error| error.prepend_path(&format!("/states/{index}")))?;
    }
    Ok(())
}

pub(crate) fn domain(
    values: &[Value],
    range: Option<crate::model::UIntRange>,
    location: Option<Location>,
) -> Result<(), Diagnostic> {
    if let Some(range) = range {
        if !values.is_empty() || range.min > range.max {
            return Err(Diagnostic::new(
                "invalid_domain",
                "Unsigned ranges require min <= max and no categorical values",
                location,
            )
            .at_path("/range"));
        }
        return Ok(());
    }
    let Some(first) = values.first() else {
        return Err(Diagnostic::new(
            "invalid_domain",
            "A domain must contain at least one value",
            location,
        )
        .at_path("/values"));
    };
    let mut seen = HashSet::new();
    for (index, value) in values.iter().enumerate() {
        if std::mem::discriminant(value) != std::mem::discriminant(first) || !seen.insert(value) {
            return Err(Diagnostic::new(
                "invalid_domain",
                "A domain requires distinct values of one type",
                location,
            )
            .at_path(format!("/values/{index}")));
        }
    }
    Ok(())
}

fn validate_state(
    state: &State,
    facts: &HashMap<&str, &Fact>,
    states: &HashMap<&str, &State>,
) -> Result<(), Diagnostic> {
    let mut assigned = HashSet::new();
    for (index, assignment) in state.values.iter().enumerate() {
        let path = format!("/values/{index}");
        let Some(domain) = facts.get(assignment.fact.as_str()) else {
            return Err(unknown("fact", &assignment.fact, assignment.location)
                .at_path(format!("{path}/fact")));
        };
        if !assigned.insert(assignment.fact.as_str()) || !domain.contains(&assignment.value) {
            return Err(Diagnostic::new(
                "invalid_assignment",
                format!(
                    "Invalid assignment for fact {:?} in state {:?}",
                    assignment.fact, state.name
                ),
                assignment.location,
            )
            .at_path(path));
        }
    }
    let mut operations = HashSet::new();
    for (index, transition) in state.transitions.iter().enumerate() {
        let path = format!("/transitions/{index}");
        if transition.operation.is_empty() {
            return Err(Diagnostic::new(
                "invalid_assignment",
                "An operation name cannot be empty",
                transition.location,
            )
            .at_path(format!("{path}/operation")));
        }
        if !operations.insert(transition.operation.as_str()) {
            return Err(Diagnostic::new(
                "duplicate_operation",
                format!(
                    "Duplicate operation {:?} in state {:?}",
                    transition.operation, state.name
                ),
                transition.location,
            )
            .at_path(format!("{path}/operation")));
        }
        if !states.contains_key(transition.target.as_str()) {
            return Err(
                unknown("target state", &transition.target, transition.location)
                    .at_path(format!("{path}/target")),
            );
        }
    }
    Ok(())
}

fn validate_hierarchy(
    document: &Document,
    branches: &HashMap<&str, &Branch>,
) -> Result<(), Diagnostic> {
    let mut parents = HashMap::new();
    for (parent_index, parent) in document.branches.iter().enumerate() {
        let mut bound_facts = HashSet::new();
        for (binding_index, binding) in parent.sub_branches.iter().enumerate() {
            let path = format!("/branches/{parent_index}/sub_branches/{binding_index}");
            let Some(child) = branches.get(binding.branch.as_str()) else {
                return Err(unknown("child branch", &binding.branch, binding.location)
                    .at_path(format!("{path}/branch")));
            };
            let Some(fact) = parent.facts.iter().find(|fact| fact.name == binding.fact) else {
                return Err(unknown("parent fact", &binding.fact, binding.location)
                    .at_path(format!("{path}/fact")));
            };
            if fact.values.len() != 2
                || !fact.values.contains(&Value::Bool(false))
                || !fact.values.contains(&Value::Bool(true))
            {
                return Err(hierarchy(
                    "A child binding requires a parent fact with domain [false, true]",
                    binding.location,
                )
                .at_path(path));
            }
            if !bound_facts.insert(&binding.fact) {
                return Err(hierarchy(
                    "A parent fact can be bound to only one child",
                    binding.location,
                )
                .at_path(path));
            }
            if parents
                .insert(child.name.as_str(), parent.name.as_str())
                .is_some()
            {
                return Err(hierarchy(
                    "A child branch must have exactly one owner",
                    binding.location,
                )
                .at_path(path));
            }
            if child.activating.is_none() {
                return Err(hierarchy(
                    "A child branch must declare an activation predicate",
                    binding.location,
                )
                .at_path(path));
            }
            if fact.initial.is_some() || fact.derived.is_some() {
                return Err(hierarchy(
                    "A bound summary cannot declare an initial value or another derivation",
                    binding.location,
                )
                .at_path(path));
            }
        }
    }
    let mut completed = HashSet::new();
    for (index, branch) in document.branches.iter().enumerate() {
        let mut ancestors = HashSet::new();
        let mut name = branch.name.as_str();
        while let Some(parent) = parents.get(name) {
            if completed.contains(name) {
                break;
            }
            if !ancestors.insert(name) {
                return Err(hierarchy(
                    "Subordinate branch bindings must form an acyclic forest",
                    branch.location,
                )
                .at_path(format!("/branches/{index}/sub_branches")));
            }
            name = parent;
        }
        completed.extend(ancestors);
    }
    Ok(())
}

pub(crate) fn unknown(kind: &str, name: &str, location: Option<Location>) -> Diagnostic {
    Diagnostic::new(
        "unknown_reference",
        format!("Unknown {kind} {name:?}"),
        location,
    )
}

fn hierarchy(message: &str, location: Option<Location>) -> Diagnostic {
    Diagnostic::new("invalid_hierarchy", message, location)
}
