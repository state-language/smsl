mod formatting;
pub(crate) mod lexer;
pub mod native;

pub use formatting::format;

use crate::{Diagnostic, encoding::Encoding};
use serde::{Deserialize, Serialize};
use std::str::FromStr;

#[derive(Clone, Copy, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(rename_all = "lowercase")]
pub enum SourceFormat {
    Smsl,
    Json,
    Yaml,
    Xml,
    Toml,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct Source {
    pub id: String,
    pub text: String,
    pub format: SourceFormat,
}

impl SourceFormat {
    pub fn encoding(self) -> Option<Encoding> {
        match self {
            Self::Smsl => None,
            Self::Json => Some(Encoding::Json),
            Self::Yaml => Some(Encoding::Yaml),
            Self::Xml => Some(Encoding::Xml),
            Self::Toml => Some(Encoding::Toml),
        }
    }
}

impl FromStr for SourceFormat {
    type Err = Diagnostic;

    fn from_str(value: &str) -> Result<Self, Self::Err> {
        match value {
            "smsl" => Ok(Self::Smsl),
            "json" => Ok(Self::Json),
            "yaml" | "yml" => Ok(Self::Yaml),
            "xml" => Ok(Self::Xml),
            "toml" => Ok(Self::Toml),
            _ => Err(Diagnostic::new(
                "invalid_encoding",
                format!("Unknown source format {value:?}"),
                None,
            )),
        }
    }
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SourceText {
    pub text: String,
    pub format: SourceFormat,
}

pub trait Resolver {
    fn resolve(&mut self, importer: &str, source: &str) -> Result<Source, Diagnostic>;
}

impl<F> Resolver for F
where
    F: FnMut(&str, &str) -> Result<Source, Diagnostic>,
{
    fn resolve(&mut self, importer: &str, source: &str) -> Result<Source, Diagnostic> {
        self(importer, source)
    }
}
