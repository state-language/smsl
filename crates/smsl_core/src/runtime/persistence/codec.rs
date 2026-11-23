use crate::{
    Diagnostic, Machine,
    encoding::{self, Encoding},
};

use crate::runtime::{Event, Snapshot, World};

pub fn decode_event(source: &str, format: Encoding) -> Result<Event, Diagnostic> {
    crate::runtime::parse_event(&decode_json(source, format)?)
}

pub fn encode_event(event: &Event, format: Encoding) -> Result<String, Diagnostic> {
    encoding::encode(&serde_json::to_value(event).expect("event values"), format)
}

pub fn decode_snapshot(
    machine: &Machine,
    source: &str,
    format: Encoding,
) -> Result<Snapshot, Diagnostic> {
    crate::runtime::restore(machine, &decode_json(source, format)?)
}

pub fn encode_snapshot(snapshot: &Snapshot, format: Encoding) -> Result<String, Diagnostic> {
    encode_json(&crate::runtime::dump_snapshot(snapshot)?, format)
}

pub fn decode_world(source: &str, format: Encoding) -> Result<World, Diagnostic> {
    World::restore(&decode_json(source, format)?)
}

pub fn encode_world(world: &World, format: Encoding) -> Result<String, Diagnostic> {
    encode_json(&world.to_json()?, format)
}

fn decode_json(source: &str, format: Encoding) -> Result<String, Diagnostic> {
    if format == Encoding::Json {
        Ok(source.to_owned())
    } else {
        Ok(serde_json::to_string(&encoding::decode(source, format)?).expect("decoded values"))
    }
}

fn encode_json(source: &str, format: Encoding) -> Result<String, Diagnostic> {
    encoding::encode(&encoding::decode(source, Encoding::Json)?, format)
}
