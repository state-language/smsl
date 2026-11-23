use crate::{Diagnostic, encoding, modules::SourceFormat, source::native};

use super::{Definition, error, validate};

pub fn decode_source(source: &str, format: SourceFormat) -> Result<Definition, Diagnostic> {
    let value = if let Some(format) = format.encoding() {
        encoding::decode(source, format)?
    } else {
        let parsed = native::parse(source)?;
        if parsed.kind != "execution" {
            return Err(error("invalid_execution", "Expected an execution document"));
        }
        parsed.value
    };
    let definition = serde_json::from_value(value)
        .map_err(|cause| error("invalid_execution", cause.to_string()))?;
    validate(&definition)?;
    Ok(definition)
}

pub fn encode_source(definition: &Definition, format: SourceFormat) -> Result<String, Diagnostic> {
    validate(definition)?;
    if let Some(format) = format.encoding() {
        super::encode(definition, format)
    } else {
        native::encode_execution(definition)
    }
}
