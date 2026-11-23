use crate::{
    Diagnostic, Machine,
    language::{
        model::Datum,
        semantics::{self, Evaluation},
    },
};

use super::{RuntimeState, validation::error};

impl RuntimeState {
    pub(crate) fn recompute(&mut self, machine: &Machine) -> Result<(), Diagnostic> {
        semantics::recompute(machine, &mut self.branches)
    }

    pub(crate) fn validate_constraints(&self, machine: &Machine) -> Result<(), Diagnostic> {
        semantics::validate_constraints(machine, &self.branches)
    }

    pub fn activation(&self, machine: &Machine, name: &str) -> Result<Datum, Diagnostic> {
        self.check_machine(machine)?;
        let index = machine
            .branch_id(name)
            .ok_or_else(|| error("unknown_reference", format!("Unknown branch {name:?}")))?;
        let branch = &machine.document().branches[index];
        let expression = branch
            .activating
            .as_ref()
            .ok_or_else(|| error("invalid_operation", "Branch has no activation predicate"))?;
        Evaluation::state(&self.branches, branch).predicate(expression)
    }
}
