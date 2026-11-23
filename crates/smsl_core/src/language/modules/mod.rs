mod codec;
mod domains;
mod linker;
mod model;
mod references;
mod sources;
mod templates;

pub use crate::source::{Resolver, Source, SourceFormat, SourceText};
pub use codec::{decode, encode};
pub use linker::{link, link_document};
pub use model::{Domain, Import, ImportedName, Instance, Module, Template};

pub use sources::link_sources;
