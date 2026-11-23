mod codec;
mod definition;
mod model;
mod profiles;
mod validation;

pub use codec::{compile_request, decode, encode, load, request};
pub use definition::ApplicationDefinition;
pub use model::*;
pub(crate) use profiles::standard;
pub use profiles::validate_profile;

fn error(message: impl Into<String>) -> crate::Diagnostic {
    crate::Diagnostic::new("invalid_application", message, None)
}
