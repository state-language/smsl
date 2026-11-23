use super::Evaluation;
use crate::{Diagnostic, model::Operation};

pub(crate) enum OperationSelection<'a> {
    Enabled(&'a Operation),
    Disabled,
    Unknown,
    Ambiguous,
}

impl Evaluation<'_> {
    pub(crate) fn select_operation<'a>(
        &self,
        operations: &'a [Operation],
        name: &str,
    ) -> Result<OperationSelection<'a>, Diagnostic> {
        use OperationSelection::*;

        let mut selection = Disabled;
        for operation in operations.iter().filter(|operation| operation.name == name) {
            match operation
                .guard
                .as_ref()
                .map_or(Ok(Some(true)), |guard| self.logical(guard))?
            {
                Some(true) if matches!(selection, Enabled(_)) => return Ok(Ambiguous),
                Some(true) => selection = Enabled(operation),
                None if matches!(selection, Disabled) => selection = Unknown,
                _ => {}
            }
        }
        Ok(selection)
    }
}
