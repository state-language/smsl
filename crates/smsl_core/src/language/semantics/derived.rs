use crate::{Diagnostic, diagnostic::key, language::Machine};

use super::{BranchValues, Evaluation, FactoredBranch, error};

pub(crate) fn recompute(machine: &Machine, branches: &mut BranchValues) -> Result<(), Diagnostic> {
    for (name, field) in machine.derived_order() {
        let branch =
            &machine.document().branches[machine.branch_id(name).expect("compiled branch")];
        let (value, label) =
            if let Some(label) = branch.labels.iter().find(|label| label.name == *field) {
                (
                    Evaluation::state(branches, branch).predicate(&label.expression)?,
                    true,
                )
            } else if let Some(state) = branch
                .named_states
                .iter()
                .find(|state| state.name == *field)
            {
                (
                    Evaluation::state(branches, branch).predicate(&state.expression)?,
                    false,
                )
            } else if let Some(binding) = branch
                .sub_branches
                .iter()
                .find(|binding| binding.fact == *field)
            {
                let child = &machine.document().branches
                    [machine.branch_id(&binding.branch).expect("compiled child")];
                (
                    Evaluation::state(branches, child)
                        .predicate(child.activating.as_ref().expect("compiled activation"))?,
                    false,
                )
            } else {
                let fact = branch
                    .facts
                    .iter()
                    .find(|fact| fact.name == *field)
                    .expect("compiled fact");
                (
                    Evaluation::state(branches, branch)
                        .evaluate(fact.derived.as_ref().expect("compiled derivation"))?,
                    false,
                )
            };
        let current = &branches[name];
        let previous = if label {
            current.labels.get(field)
        } else if branch.named_states.iter().any(|state| state.name == *field) {
            current.memberships.get(field)
        } else {
            current.facts.get(field)
        };
        if previous == Some(&value) {
            continue;
        }
        let current = FactoredBranch::edit(branches.get_mut(name).expect("validated branch"));
        if label {
            current.labels.insert(field.clone(), value);
        } else if branch.named_states.iter().any(|state| state.name == *field) {
            current.memberships.insert(field.clone(), value);
        } else {
            current.facts.insert(field.clone(), value);
        }
    }
    Ok(())
}

pub(crate) fn validate_constraints(
    machine: &Machine,
    branches: &BranchValues,
) -> Result<(), Diagnostic> {
    for (index, branch) in machine.document().branches.iter().enumerate() {
        for invariant in &machine.programs[index].invariants {
            if !Evaluation::state(branches, branch).boolean(&invariant.expression)? {
                return Err(error(
                    "invalid_snapshot",
                    format!("Invariant {:?} must be established", invariant.name),
                )
                .at_path(format!(
                    "/branches/{}/invariants/{}",
                    key(&branch.name),
                    key(&invariant.name)
                )));
            }
        }
    }
    Ok(())
}
