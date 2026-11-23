use std::sync::Arc;

use crate::{
    Diagnostic,
    language::{Machine, model::Datum},
};

use super::{
    BranchSnapshot, BranchValues, FactoredBranch, error, recompute, validate_constraints,
    validate_values,
};

pub(crate) fn initial(machine: &Machine) -> Result<BranchValues, Diagnostic> {
    let mut branches = machine
        .document()
        .branches
        .iter()
        .map(|branch| {
            (
                branch.name.clone(),
                Arc::new(FactoredBranch::from_snapshot(BranchSnapshot {
                    state: branch.initial.clone(),
                    memberships: branch
                        .named_states
                        .iter()
                        .map(|state| (state.name.clone(), Datum::unknown()))
                        .collect(),
                    facts: branch
                        .facts
                        .iter()
                        .map(|fact| {
                            (
                                fact.name.clone(),
                                fact.initial.clone().unwrap_or_else(Datum::unknown),
                            )
                        })
                        .collect(),
                    labels: branch
                        .labels
                        .iter()
                        .map(|label| (label.name.clone(), Datum::unknown()))
                        .collect(),
                    data: branch
                        .data
                        .iter()
                        .map(|field| (field.name.clone(), field.initial.clone()))
                        .collect(),
                })),
            )
        })
        .collect();
    recompute(machine, &mut branches)?;
    validate_values(machine, &branches, &|_| {
        Err(error(
            "invalid_snapshot",
            "Event reference must identify an already consumed local step or retained World event",
        ))
    })?;
    validate_constraints(machine, &branches)?;
    Ok(branches)
}
