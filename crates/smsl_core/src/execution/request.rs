use serde::Deserialize;
use serde_json::Value;

use super::{Definition, Input, Session, decode_source, encode_source, error, validate};
use crate::{
    Diagnostic,
    encoding::{self, Encoding},
    modules::SourceFormat,
};

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Decode {
        source: String,
        format: SourceFormat,
    },
    Encode {
        definition: Definition,
        format: SourceFormat,
    },
    Validate {
        definition: Definition,
    },
}

pub fn request(source: &str) -> Result<String, Diagnostic> {
    let request: Request = parse(source)?;
    let value = match request {
        Request::Decode { source, format } => serde_json::to_value(decode_source(&source, format)?),
        Request::Encode { definition, format } => {
            serde_json::to_value(encode_source(&definition, format)?)
        }
        Request::Validate { definition } => {
            validate(&definition)?;
            Ok(Value::Null)
        }
    }
    .expect("execution response serialization");
    Ok(value.to_string())
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum SessionRequest {
    Apply { input: Input },
    Inspect {},
    Save { format: String },
    World { format: SourceFormat },
}

impl Session {
    pub fn request(&mut self, source: &str) -> Result<String, Diagnostic> {
        let value = match parse(source)? {
            SessionRequest::Apply { input } => serde_json::to_value(self.apply(input)?),
            SessionRequest::Inspect {} => Ok(serde_json::json!({"status": self.status(), "states": self.states(), "definition": self.definition()})),
            SessionRequest::Save { format } => serde_json::to_value(self.save(format.parse()?)?),
            SessionRequest::World { format } => serde_json::to_value(crate::memory::dump_world(self.world(), format)?),
        }.expect("execution session response serialization");
        Ok(value.to_string())
    }
}

fn parse<T: serde::de::DeserializeOwned>(source: &str) -> Result<T, Diagnostic> {
    serde_json::from_value(encoding::decode(source, Encoding::Json)?)
        .map_err(|cause| error("execution_request", cause.to_string()))
}
