mod basis;
mod catalog;
mod driver;
mod provider;
mod records;
pub mod remote;
mod session;

use crate::architecture::{Contract, Freshness, Port, SystemDefinition, validate};

pub use catalog::{
    ProviderCatalog, ProviderDescription, ProviderMetadata, ProviderPersistence, describe_provider,
};
pub use driver::{ProviderCommand, ProviderDriver};
pub use provider::{
    BasisReader, OutputValidator, Provider, ProviderHooks, ProviderRegistry, ProviderSession,
    Runtime,
};
pub use records::*;
pub use session::{
    InvocationPhase, InvocationStatus, LifecycleEntry, LifecycleKind, LifecyclePage,
    LifecycleStatus, Session,
};

fn error(code: &str, message: impl Into<String>) -> crate::Diagnostic {
    crate::Diagnostic::new(code, message, None)
}
