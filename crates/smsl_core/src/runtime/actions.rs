use crate::language::semantics::Evaluation;

use std::collections::BTreeMap;

use crate::{
    Diagnostic, Machine,
    model::{Datum, EventField},
};

use super::{
    RuntimeState,
    validation::{self, Resolver, error},
};

pub fn applicable(
    machine: &Machine,
    state: &RuntimeState,
    branch: &str,
    action: &str,
    args: &BTreeMap<String, Datum>,
) -> Result<bool, Diagnostic> {
    state.check_standalone()?;
    applicable_with(machine, state, branch, action, args, &|_| false)
}

pub(crate) fn applicable_with(
    machine: &Machine,
    state: &RuntimeState,
    name: &str,
    action: &str,
    args: &BTreeMap<String, Datum>,
    resolve: &Resolver<'_>,
) -> Result<bool, Diagnostic> {
    state.check_machine(machine)?;
    let index = machine
        .branch_id(name)
        .ok_or_else(|| error("unknown_reference", format!("Unknown branch {name:?}")))?;
    let branch = &machine.document().branches[index];
    let action = branch
        .actions
        .iter()
        .find(|candidate| candidate.name == action)
        .ok_or_else(|| error("unknown_reference", format!("Unknown action {action:?}")))?;
    arguments(&action.parameters, args, state, resolve)?;
    action
        .precondition
        .as_ref()
        .map_or(Ok(true), |precondition| {
            Evaluation {
                args: Some(args),
                ..Evaluation::state(&state.branches, branch)
            }
            .boolean(precondition)
        })
}

pub(crate) fn arguments(
    fields: &[EventField],
    args: &BTreeMap<String, Datum>,
    state: &RuntimeState,
    resolve: &Resolver<'_>,
) -> Result<(), Diagnostic> {
    if args.len() != fields.len() {
        return Err(error(
            "invalid_arguments",
            "Action arguments must contain exactly the declared parameters",
        ));
    }
    for field in fields {
        let path = format!("/args/{}", validation::key(&field.name));
        let value = args
            .get(&field.name)
            .ok_or_else(|| error("invalid_arguments", "Missing action argument").at_path(&path))?;
        if matches!(value, Datum::Unknown(_))
            || value
                .data_type()
                .map_or(!field.nullable, |_| !field.data_type.accepts(value))
        {
            return Err(error(
                "invalid_arguments",
                "Incorrect argument type or nullability",
            )
            .at_path(path));
        }
        if let Datum::Event(reference) = value {
            validation::reference(
                reference,
                &state.context.scope,
                state.step,
                "invalid_arguments",
                resolve,
            )?;
        }
    }
    Ok(())
}
