mod derived;
mod evaluate;
mod operation;
mod types;
mod validation;
mod values;

pub(crate) use derived::{recompute, validate_constraints};
pub(crate) use evaluate::Evaluation;
pub(crate) use operation::OperationSelection;
pub use types::{BranchSnapshot, Context, Event};
pub(crate) use types::{BranchValues, FactoredBranch};
pub(crate) use validation::validate_values;
pub(crate) use values::initial;

fn error(code: &str, message: impl Into<String>) -> crate::Diagnostic {
    crate::Diagnostic::new(code, message, None)
}
