use crate::{
    Diagnostic,
    diagnostic::key,
    language::{
        Machine,
        model::{Datum, EventRef},
    },
};

use super::{BranchValues, error};

pub(crate) fn validate_values(
    machine: &Machine,
    branches: &BranchValues,
    validate_reference: &impl Fn(&EventRef) -> Result<(), Diagnostic>,
) -> Result<(), Diagnostic> {
    let code = "invalid_snapshot";
    if branches.len() != machine.document().branches.len() {
        return Err(
            error(code, "Snapshot must contain exactly the declared branches").at_path("/branches"),
        );
    }
    for branch in &machine.document().branches {
        let path = format!("/branches/{}", key(&branch.name));
        let current = branches
            .get(&branch.name)
            .ok_or_else(|| error(code, "Missing declared branch").at_path(&path))?;
        match current.control_state() {
            Some(name) if branch.states.iter().any(|state| state.name == name) => {}
            None if branch.states.is_empty() => {}
            _ => {
                return Err(error(code, "Missing or unknown control state")
                    .at_path(format!("{path}/state")));
            }
        }
        for (name, actual, expected) in [
            (
                "facts",
                current.facts.len(),
                branch.facts.len() + usize::from(branch.initial.is_some()),
            ),
            ("data", current.data.len(), branch.data.len()),
            ("labels", current.labels.len(), branch.labels.len()),
            (
                "memberships",
                current.memberships.len(),
                branch.named_states.len(),
            ),
        ] {
            if actual != expected {
                return Err(
                    error(code, "Snapshot fields must match the branch declarations")
                        .at_path(format!("{path}/{name}")),
                );
            }
        }
        for fact in &branch.facts {
            let field_path = format!("{path}/facts/{}", key(&fact.name));
            let value = current
                .facts
                .get(&fact.name)
                .ok_or_else(|| error(code, "Missing fact").at_path(&field_path))?;
            if !fact.accepts(value) {
                return Err(
                    error(code, "Fact value is outside its declared domain").at_path(field_path)
                );
            }
        }
        for label in &branch.labels {
            if !matches!(
                current.labels.get(&label.name),
                Some(Datum::Bool(_) | Datum::Unknown(_))
            ) {
                return Err(error(code, "A label must be Boolean or unknown")
                    .at_path(format!("{path}/labels/{}", key(&label.name))));
            }
        }
        for state in &branch.named_states {
            if !matches!(
                current.memberships.get(&state.name),
                Some(Datum::Bool(_) | Datum::Unknown(_))
            ) {
                return Err(
                    error(code, "Named state membership must be Boolean or unknown")
                        .at_path(format!("{path}/memberships/{}", key(&state.name))),
                );
            }
        }
        for field in &branch.data {
            let field_path = format!("{path}/data/{}", key(&field.name));
            let value = current
                .data
                .get(&field.name)
                .ok_or_else(|| error(code, "Missing data field").at_path(&field_path))?;
            if matches!(value, Datum::Unknown(_))
                || value
                    .data_type()
                    .is_some_and(|_| !field.data_type.accepts(value))
            {
                return Err(error(code, "Incorrect data type").at_path(field_path));
            }
            if let Datum::Event(reference) = value {
                validate_reference(reference).map_err(|error| error.at_path(field_path))?;
            }
        }
    }
    Ok(())
}
