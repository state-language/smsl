mod codec;
mod definition;
mod engine;
mod lifecycle;
mod request;
mod session;
mod source;
mod transaction;
mod validation;

pub use codec::{decode, encode};
pub use definition::{Block, Definition, Mode, Status};
pub use request::request;
pub use session::{Command, Input, Report, Session};
pub use source::{decode_source, encode_source};
pub use validation::validate;

fn error(code: &str, message: impl Into<String>) -> crate::Diagnostic {
    crate::Diagnostic::new(code, message, None)
}
