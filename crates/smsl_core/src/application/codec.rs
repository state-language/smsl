use std::collections::BTreeMap;

use serde::Deserialize;
use serde_json::{Map, Value};

use super::{
    ApplicationDefinition, ApplicationDocument, ApplicationProfileDeclaration, error, validation,
};
use crate::{
    Diagnostic,
    encoding::{self, Encoding},
    language::{self, model::Document},
    source::{SourceFormat, SourceText},
};

pub fn decode(source: &str, format: SourceFormat) -> Result<ApplicationDocument, Diagnostic> {
    crate::limits::input(source, format == SourceFormat::Smsl)?;
    let value = match format.encoding() {
        Some(format) => encoding::decode(source, format)?,
        None => {
            let parsed = crate::source::native::parse(source)?;
            if parsed.kind != "application" {
                return Err(error("Expected an application declaration"));
            }
            parsed.value
        }
    };
    let document = serde_json::from_value(value).map_err(|cause| error(cause.to_string()))?;
    validation::document(&document)?;
    Ok(document)
}

pub fn encode(document: &ApplicationDocument, format: SourceFormat) -> Result<String, Diagnostic> {
    validation::document(document)?;
    let value = serde_json::to_value(document).expect("application document");
    let source = match format.encoding() {
        Some(format) => encoding::encode(&value, format)?,
        None => crate::source::native::encode_application(document)?,
    };
    crate::limits::input(&source, format == SourceFormat::Smsl)?;
    Ok(source)
}

pub fn load(
    source: &str,
    format: SourceFormat,
    sources: &BTreeMap<String, SourceText>,
    profiles: &[ApplicationProfileDeclaration],
) -> Result<ApplicationDefinition, Diagnostic> {
    ApplicationDefinition::new(decode(source, format)?, sources, profiles)
}

#[derive(Deserialize)]
#[serde(tag = "operation", rename_all = "snake_case", deny_unknown_fields)]
enum Request {
    Decode {
        source: String,
        format: SourceFormat,
    },
    Encode {
        document: ApplicationDocument,
        format: SourceFormat,
    },
    Compile {
        document: ApplicationDocument,
        #[serde(default)]
        sources: BTreeMap<String, SourceText>,
        #[serde(default)]
        profiles: Vec<ApplicationProfileDeclaration>,
    },
    Load {
        source: String,
        format: SourceFormat,
        #[serde(default)]
        sources: BTreeMap<String, SourceText>,
        #[serde(default)]
        profiles: Vec<ApplicationProfileDeclaration>,
    },
    ValidateProfile {
        profile: String,
        definition: Document,
        config: Map<String, Value>,
    },
}

fn parse_request(source: &str) -> Result<Request, Diagnostic> {
    let value = encoding::decode(source, Encoding::Json)?;
    serde_json::from_value(value).map_err(|cause| error(cause.to_string()))
}

fn compile(request: Request) -> Result<ApplicationDefinition, Diagnostic> {
    match request {
        Request::Compile {
            document,
            sources,
            profiles,
        } => ApplicationDefinition::new(document, &sources, &profiles),
        Request::Load {
            source,
            format,
            sources,
            profiles,
        } => load(&source, format, &sources, &profiles),
        _ => Err(error("Expected a compile or load application request")),
    }
}

pub fn compile_request(source: &str) -> Result<ApplicationDefinition, Diagnostic> {
    compile(parse_request(source)?)
}

pub fn request(source: &str) -> Result<String, Diagnostic> {
    let result = match parse_request(source)? {
        Request::Decode { source, format } => serde_json::to_value(decode(&source, format)?),
        Request::Encode { document, format } => serde_json::to_value(encode(&document, format)?),
        request @ (Request::Compile { .. } | Request::Load { .. }) => {
            serde_json::to_value(compile(request)?.resolved())
        }
        Request::ValidateProfile {
            profile,
            definition,
            config,
        } => {
            super::validate_profile(&profile, &language::compile(definition)?, &config)?;
            Ok(Value::Null)
        }
    }
    .expect("application response serialization");
    encoding::encode(&result, Encoding::Json)
}
