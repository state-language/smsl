use serde::{Deserialize, Serialize};

use crate::{
    Diagnostic,
    encoding::{self, Encoding},
    runtime::World,
};

use super::{Definition, Input, Session, error, validate};

pub fn decode(source: &str, format: Encoding) -> Result<Definition, Diagnostic> {
    let definition = serde_json::from_value(encoding::decode(source, format)?)
        .map_err(|cause| error("invalid_execution", cause.to_string()))?;
    validate(&definition)?;
    Ok(definition)
}

pub fn encode(definition: &Definition, format: Encoding) -> Result<String, Diagnostic> {
    validate(definition)?;
    encoding::encode(
        &serde_json::to_value(definition).expect("execution definition serialization"),
        format,
    )
}

#[derive(Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct Archive {
    version: u64,
    definition: Definition,
    initial_world: serde_json::Value,
    records: Vec<Input>,
}

impl Session {
    pub fn save(&self, format: Encoding) -> Result<String, Diagnostic> {
        if !self.pending.is_empty() {
            return Err(error(
                "pending_action",
                "Saving execution requires completed or acknowledged cancelled actions",
            ));
        }
        let archive = Archive {
            version: 1,
            definition: self.definition.clone(),
            initial_world: self.initial_world.clone(),
            records: self.records.clone(),
        };
        encoding::encode(
            &serde_json::to_value(archive).expect("execution archive serialization"),
            format,
        )
    }

    pub fn restore(source: &str, format: Encoding) -> Result<Self, Diagnostic> {
        let archive: Archive = serde_json::from_value(encoding::decode(source, format)?)
            .map_err(|cause| error("invalid_execution", cause.to_string()))?;
        if archive.version != 1 {
            return Err(error(
                "execution_version",
                "Unsupported execution archive version",
            ));
        }
        let world = World::restore(&encoding::encode(&archive.initial_world, Encoding::Json)?)?;
        let mut session = Self::new(archive.definition, world)?;
        for (index, input) in archive.records.into_iter().enumerate() {
            session
                .apply(input)
                .map_err(|cause| cause.prepend_path(&format!("/records/{index}")))?;
        }
        if !session.pending.is_empty() {
            return Err(error(
                "pending_action",
                "Execution archive ends with pending external actions",
            ));
        }
        Ok(session)
    }
}
