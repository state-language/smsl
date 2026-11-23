mod compression;
mod text;

use serde_json::Value;

use crate::{
    Diagnostic, Machine, encoding,
    modules::SourceFormat,
    runtime::{self, Snapshot, World},
};

pub use compression::{compress, decompress};

pub(crate) fn validate_native(kind: &str, source: &str) -> Result<(), Diagnostic> {
    text::decode(kind, source).map(|_| ())
}

pub fn dump_world(world: &World, format: SourceFormat) -> Result<String, Diagnostic> {
    encode(
        "world",
        &encoding::decode(&world.to_json()?, encoding::Encoding::Json)?,
        format,
    )
}

pub fn restore_world(source: &str, format: SourceFormat) -> Result<World, Diagnostic> {
    let value = decode("world", source, format)?;
    World::restore(&serde_json::to_string(&value).expect("decoded memory value"))
}

pub fn dump_snapshot(snapshot: &Snapshot, format: SourceFormat) -> Result<String, Diagnostic> {
    encode(
        "snapshot",
        &encoding::decode(&runtime::dump_snapshot(snapshot)?, encoding::Encoding::Json)?,
        format,
    )
}

pub fn restore_snapshot(
    machine: &Machine,
    source: &str,
    format: SourceFormat,
) -> Result<Snapshot, Diagnostic> {
    let value = decode("snapshot", source, format)?;
    runtime::restore(
        machine,
        &serde_json::to_string(&value).expect("decoded memory value"),
    )
}

fn encode(kind: &str, value: &Value, format: SourceFormat) -> Result<String, Diagnostic> {
    match format.encoding() {
        Some(format) => encoding::encode(value, format),
        None => text::encode(kind, value),
    }
}

fn decode(kind: &str, source: &str, format: SourceFormat) -> Result<Value, Diagnostic> {
    match format.encoding() {
        Some(format) => encoding::decode(source, format),
        None => text::decode(kind, source),
    }
}
