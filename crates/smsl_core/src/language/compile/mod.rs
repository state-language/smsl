pub(crate) mod monitor;
mod normalize;

use super::{Machine, machine::IndexedBranch, model::Document, validation::validate};
use crate::Diagnostic;
use normalize::{infer_reads, normalize};
use sha2::{Digest, Sha256};
use std::{
    collections::{BTreeSet, HashMap},
    sync::Arc,
};

pub fn compile(mut document: Document) -> Result<Machine, Diagnostic> {
    crate::language::limits::document(&mut document)?;
    validate(&document)?;
    infer_reads(&mut document);
    crate::language::limits::document(&mut document)?;
    normalize(&mut document);
    let derived_order = crate::language::validation::derived::order(&document)?;
    let names: HashMap<_, _> = document
        .branches
        .iter()
        .enumerate()
        .map(|(index, branch)| (branch.name.clone(), index))
        .collect();
    let branches = document
        .branches
        .iter()
        .map(|branch| {
            let states: HashMap<_, _> = branch
                .states
                .iter()
                .enumerate()
                .map(|(index, state)| (state.name.clone(), index))
                .collect();
            let facts: HashMap<_, _> = branch
                .facts
                .iter()
                .enumerate()
                .map(|(index, fact)| (fact.name.clone(), index))
                .collect();
            let operations: HashMap<_, _> = branch
                .states
                .iter()
                .flat_map(|state| {
                    state
                        .transitions
                        .iter()
                        .map(|transition| transition.operation.clone())
                })
                .collect::<BTreeSet<_>>()
                .into_iter()
                .enumerate()
                .map(|(index, operation)| (operation, index))
                .collect();
            let edges = branch
                .states
                .iter()
                .map(|state| {
                    state
                        .transitions
                        .iter()
                        .map(|transition| {
                            (
                                operations[&transition.operation],
                                states[&transition.target],
                            )
                        })
                        .collect()
                })
                .collect();
            let values = branch
                .states
                .iter()
                .map(|state| {
                    branch
                        .facts
                        .iter()
                        .map(|fact| {
                            state
                                .values
                                .iter()
                                .find(|assignment| assignment.fact == fact.name)
                                .and_then(|assignment| {
                                    fact.values
                                        .iter()
                                        .position(|value| value == &assignment.value)
                                })
                        })
                        .collect()
                })
                .collect();
            let initial = branch.initial.as_ref().map(|name| states[name]);
            let bindings = branch
                .sub_branches
                .iter()
                .map(|binding| (facts[&binding.fact], names[&binding.branch]))
                .collect();
            IndexedBranch {
                states,
                facts,
                operations,
                edges,
                values,
                initial,
                bindings,
            }
        })
        .collect();
    let programs = document.branches.iter().map(monitor::lower).collect();
    let mut machine = Machine {
        programs,
        document,
        canonical_source: Arc::from(""),
        names,
        branches,
        derived_order,
        revision: String::new(),
    };
    machine.canonical_source = Arc::from(crate::to_smsl(&machine));
    crate::source::lexer::validate_source(&machine.canonical_source)?;
    machine.revision = format!("{:x}", Sha256::digest(machine.canonical_source.as_bytes()));
    super::semantics::initial(&machine)?;
    Ok(machine)
}
