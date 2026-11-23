use std::collections::BTreeSet;

use crate::language::model::{Document, ReadBinding, Value};

pub(super) fn normalize(document: &mut Document) {
    document
        .events
        .sort_by(|left, right| left.name.cmp(&right.name));
    for event in &mut document.events {
        event
            .fields
            .sort_by(|left, right| left.name.cmp(&right.name));
    }
    document
        .branches
        .sort_by(|left, right| left.name.cmp(&right.name));
    for branch in &mut document.branches {
        branch.location = None;
        branch
            .facts
            .sort_by(|left, right| left.name.cmp(&right.name));
        for fact in &mut branch.facts {
            fact.location = None;
            fact.values.sort_by(|left, right| match (left, right) {
                (Value::Bool(left), Value::Bool(right)) => left.cmp(right),
                (Value::String(left), Value::String(right)) => left.cmp(right),
                (Value::UInt(left), Value::UInt(right)) => left.cmp(right),
                _ => unreachable!("Validated domains have homogeneous values"),
            });
        }
        branch
            .sub_branches
            .sort_by(|left, right| left.branch.cmp(&right.branch));
        for binding in &mut branch.sub_branches {
            binding.location = None;
        }
        branch
            .data
            .sort_by(|left, right| left.name.cmp(&right.name));
        for field in &mut branch.data {
            field.location = None;
        }
        branch
            .reads
            .sort_by(|left, right| left.alias.cmp(&right.alias));
        for binding in &mut branch.reads {
            binding.location = None;
        }
        branch
            .states
            .sort_by(|left, right| left.name.cmp(&right.name));
        branch
            .labels
            .sort_by(|left, right| left.name.cmp(&right.name));
        branch
            .actions
            .sort_by(|left, right| left.name.cmp(&right.name));
        branch
            .abstractions
            .sort_by(|left, right| left.name.cmp(&right.name));
        branch.named_states.sort_by(|a, b| a.name.cmp(&b.name));
        branch.invariants.sort_by(|a, b| a.name.cmp(&b.name));
        branch.operations.sort_by(|a, b| a.name.cmp(&b.name));
        for operation in &mut branch.operations {
            operation.updates.sort_by(|a, b| a.field.cmp(&b.field));
        }
        for abstraction in &mut branch.abstractions {
            abstraction.facts.sort();
        }
        for action in &mut branch.actions {
            action
                .parameters
                .sort_by(|left, right| left.name.cmp(&right.name));
            action.outcomes.sort();
        }
        for reaction in &mut branch.reactions {
            reaction.location = None;
            reaction
                .updates
                .sort_by(|left, right| left.field.cmp(&right.field));
        }
        for state in &mut branch.states {
            state.location = None;
            state
                .values
                .sort_by(|left, right| left.fact.cmp(&right.fact));
            for value in &mut state.values {
                value.location = None;
            }
            state
                .transitions
                .sort_by(|left, right| left.operation.cmp(&right.operation));
            for transition in &mut state.transitions {
                transition.location = None;
                transition
                    .updates
                    .sort_by(|left, right| left.field.cmp(&right.field));
            }
            for reaction in &mut state.reactions {
                reaction.location = None;
                reaction
                    .updates
                    .sort_by(|left, right| left.field.cmp(&right.field));
            }
        }
    }
}

pub(super) fn infer_reads(document: &mut Document) {
    let names: BTreeSet<_> = document
        .branches
        .iter()
        .map(|branch| branch.name.clone())
        .collect();
    for branch in &mut document.branches {
        let sources: BTreeSet<_> = crate::language::validation::derived::expressions(branch)
            .into_iter()
            .flat_map(crate::language::validation::derived::reads)
            .map(|(source, _)| source.to_owned())
            .collect();
        for source in sources {
            if source != branch.name
                && names.contains(&source)
                && !branch.reads.iter().any(|binding| binding.alias == source)
            {
                branch.reads.push(ReadBinding {
                    alias: source.clone(),
                    branch: source,
                    location: None,
                });
            }
        }
    }
}
