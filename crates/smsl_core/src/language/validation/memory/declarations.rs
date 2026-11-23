use std::collections::{HashMap, HashSet};

use super::error;
use crate::diagnostic::Diagnostic;
use crate::language::model::{Branch, Datum, EventDeclaration, EventField};
use crate::language::validation::unknown;

pub(super) fn branch(branch: &Branch, branches: &HashMap<&str, &Branch>) -> Result<(), Diagnostic> {
    if matches!(branch.name.as_str(), "self" | "event" | "payload" | "args") {
        return Err(error(
            "invalid_data",
            "Reserved expression source cannot name a branch",
            branch.location,
        ));
    }
    let mut names = HashSet::from(["state"]);
    for (index, fact) in branch.facts.iter().enumerate() {
        if !names.insert(fact.name.as_str()) {
            return Err(error(
                "duplicate_name",
                format!("Reserved or duplicate fact {:?}", fact.name),
                fact.location,
            )
            .at_path(format!("/facts/{index}/name")));
        }
        let bound = branch
            .sub_branches
            .iter()
            .any(|binding| binding.fact == fact.name);
        if fact.derived.is_some() || bound {
            if fact.initial.is_some() {
                return Err(error(
                    "invalid_assignment",
                    "Derived facts cannot have initial values",
                    fact.location,
                )
                .at_path(format!("/facts/{index}/initial")));
            }
        } else if !fact
            .initial
            .as_ref()
            .is_some_and(|value| fact.accepts(value))
        {
            return Err(error(
                "invalid_assignment",
                "A stored fact requires an initial domain value or unknown",
                fact.location,
            )
            .at_path(format!("/facts/{index}/initial")));
        }
    }
    for (index, field) in branch.data.iter().enumerate() {
        let path = format!("/data/{index}");
        if field.name.is_empty()
            || field.name == "state"
            || branch.facts.iter().any(|fact| fact.name == field.name)
        {
            return Err(error(
                "invalid_data",
                format!("Empty, reserved or overlapping data field {:?}", field.name),
                field.location,
            )
            .at_path(path));
        }
        if !names.insert(field.name.as_str()) {
            return Err(error(
                "duplicate_name",
                format!("Duplicate data field {:?}", field.name),
                field.location,
            )
            .at_path(path));
        }
        if matches!(field.initial, Datum::Event(_) | Datum::Unknown(_))
            || field
                .initial
                .data_type()
                .is_some_and(|_| !field.data_type.accepts(&field.initial))
        {
            return Err(error(
                "invalid_data",
                format!("Invalid initial value for {:?}", field.name),
                field.location,
            )
            .at_path(path));
        }
    }
    for (index, label) in branch.labels.iter().enumerate() {
        if label.name.is_empty() || !names.insert(label.name.as_str()) {
            return Err(error(
                "duplicate_name",
                "Predicate labels require distinct nonempty field names",
                branch.location,
            )
            .at_path(format!("/labels/{index}/name")));
        }
    }
    for (index, state) in branch.named_states.iter().enumerate() {
        if state.name.is_empty()
            || !names.insert(state.name.as_str())
            || branch
                .states
                .iter()
                .any(|control| control.name == state.name)
        {
            return Err(error(
                "duplicate_name",
                "Named states require distinct nonempty field names",
                branch.location,
            )
            .at_path(format!("/named_states/{index}/name")));
        }
    }
    for (group, entries) in [
        (
            "invariants",
            branch
                .invariants
                .iter()
                .map(|p| &p.name)
                .collect::<Vec<_>>(),
        ),
        (
            "operations",
            branch.operations.iter().map(|p| &p.name).collect(),
        ),
    ] {
        let mut unique = HashSet::new();
        for (index, name) in entries.into_iter().enumerate() {
            if name.is_empty() || !unique.insert(name) {
                return Err(error(
                    "duplicate_name",
                    "Names must be nonempty and distinct",
                    branch.location,
                )
                .at_path(format!("/{group}/{index}/name")));
            }
            if group == "operations"
                && branch
                    .states
                    .iter()
                    .flat_map(|state| &state.transitions)
                    .any(|transition| transition.operation == *name)
            {
                return Err(error(
                    "duplicate_operation",
                    "Branch operations cannot overlap control transition names",
                    branch.location,
                )
                .at_path(format!("/{group}/{index}/name")));
            }
        }
    }
    let mut aliases = HashSet::new();
    for (index, binding) in branch.reads.iter().enumerate() {
        let path = format!("/reads/{index}");
        if binding.alias.is_empty()
            || matches!(
                binding.alias.as_str(),
                "self" | "event" | "payload" | "args"
            )
            || binding.branch == branch.name
            || binding.alias == branch.name
            || branches.contains_key(binding.alias.as_str()) && binding.alias != binding.branch
        {
            return Err(error(
                "invalid_data",
                format!("Invalid or ambiguous read alias {:?}", binding.alias),
                binding.location,
            )
            .at_path(path));
        }
        if !aliases.insert(&binding.alias) {
            return Err(error(
                "duplicate_name",
                format!("Duplicate read alias {:?}", binding.alias),
                binding.location,
            )
            .at_path(path));
        }
        if !branches.contains_key(binding.branch.as_str()) {
            return Err(unknown("read branch", &binding.branch, binding.location)
                .at_path(format!("{path}/branch")));
        }
    }
    let mut actions = HashSet::new();
    for (index, action) in branch.actions.iter().enumerate() {
        let path = format!("/actions/{index}");
        if action.name.is_empty() || !actions.insert(&action.name) {
            return Err(error(
                "duplicate_name",
                "Action names must be nonempty and distinct",
                branch.location,
            )
            .at_path(path));
        }
        fields(&action.parameters)
            .map_err(|error| error.prepend_path(&format!("{path}/parameters")))?;
        let mut outcomes = HashSet::new();
        if action
            .outcomes
            .iter()
            .any(|outcome| outcome.is_empty() || !outcomes.insert(outcome))
        {
            return Err(error(
                "invalid_action",
                "Action outcomes must be nonempty and distinct",
                branch.location,
            )
            .at_path(format!("{path}/outcomes")));
        }
    }
    Ok(())
}

pub(super) fn events(events: &[EventDeclaration]) -> Result<(), Diagnostic> {
    let mut names = HashSet::new();
    for (index, event) in events.iter().enumerate() {
        let path = format!("/events/{index}");
        if event.name.is_empty() || !names.insert(&event.name) {
            return Err(error(
                "invalid_event_declaration",
                "Event names must be nonempty and distinct",
                None,
            )
            .at_path(format!("{path}/name")));
        }
        fields(&event.fields).map_err(|error| error.prepend_path(&format!("{path}/fields")))?;
    }
    Ok(())
}

fn fields(fields: &[EventField]) -> Result<(), Diagnostic> {
    let mut names = HashSet::new();
    for (index, field) in fields.iter().enumerate() {
        if field.name.is_empty() || !names.insert(&field.name) {
            return Err(error(
                "invalid_event_declaration",
                "Parameter fields must be nonempty and distinct",
                None,
            )
            .at_path(format!("/{index}/name")));
        }
    }
    Ok(())
}
