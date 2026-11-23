mod catalog;
mod codec;
mod composition;
mod contracts;
mod roles;
pub(crate) mod validation;

pub use catalog::catalog;
pub use codec::{decode, encode, link, link_sources, request};
pub use composition::*;
pub use contracts::*;
pub use roles::{Role, roles};
pub use validation::{Inspection, validate};

fn error(code: &str, message: impl Into<String>) -> crate::Diagnostic {
    crate::Diagnostic::new(code, message, None)
}
