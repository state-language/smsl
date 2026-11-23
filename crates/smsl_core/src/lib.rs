pub mod application;
pub mod architecture;
pub mod diagnostic;
pub mod encoding;
pub mod execution;
pub mod exploration;
mod finite;
pub mod language;
pub mod limits;
pub mod planning;
pub mod runtime;
pub mod source;

pub use diagnostic::{Diagnostic, Location};
pub use language::{
    Machine, compile, definition, from_json, graph, load, model, modules, parse, parse_module,
    to_json, to_smsl, to_smsl_module,
};
pub use runtime::persistence::memory;

pub mod system {
    pub use crate::architecture::*;
    pub use crate::runtime::system::*;
}

pub const LANGUAGE_VERSION: &str = "0.2";
pub const PACKAGE_VERSION: &str = env!("CARGO_PKG_VERSION");
