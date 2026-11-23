pub mod codec;
mod compile;
pub mod definition;
pub mod graph;
pub(crate) mod limits;
mod machine;
pub mod model;
pub mod modules;
pub(crate) mod semantics;
pub mod syntax;
pub(crate) mod validation;

pub use codec::{from_json, to_json};
pub use compile::compile;
pub use machine::Machine;
pub use syntax::{parse, parse_module, to_smsl, to_smsl_module};

pub fn load(source: &str) -> Result<Machine, crate::Diagnostic> {
    compile(parse(source)?)
}
