mod serde;
pub(crate) use self::serde::{unique_map, unique_optional_map};
pub use crate::language::codec::{from_encoding, to_encoding};

mod json;
mod toml;
mod xml;
mod yaml;

use std::str::FromStr;

use serde_json::Value;

use crate::{Diagnostic, limits::MAX_INPUT_BYTES};

pub const MAX_VALUE_DEPTH: usize = 96;
pub const MAX_VALUE_NODES: usize = 262_144;

#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum Encoding {
    Json,
    Yaml,
    Xml,
    Toml,
}

impl FromStr for Encoding {
    type Err = Diagnostic;

    fn from_str(name: &str) -> Result<Self, Self::Err> {
        match name {
            "json" => Ok(Self::Json),
            "yaml" => Ok(Self::Yaml),
            "xml" => Ok(Self::Xml),
            "toml" => Ok(Self::Toml),
            _ => Err(Diagnostic::new(
                "invalid_encoding",
                format!("Unknown encoding {name:?}"),
                None,
            )),
        }
    }
}

pub fn decode(source: &str, encoding: Encoding) -> Result<Value, Diagnostic> {
    size(source)?;
    let value = match encoding {
        Encoding::Json => json::decode(source),
        Encoding::Yaml => yaml::decode(source),
        Encoding::Xml => xml::decode(source),
        Encoding::Toml => toml::decode(source),
    }?;
    validate_value(&value)?;
    Ok(value)
}

pub fn encode(value: &Value, encoding: Encoding) -> Result<String, Diagnostic> {
    validate_value(value)?;
    let source = match encoding {
        Encoding::Json => serde_json::to_string_pretty(value).expect("validated value tree") + "\n",
        Encoding::Yaml => yaml::encode(value),
        Encoding::Xml => xml::encode(value),
        Encoding::Toml => toml::encode(value),
    };
    size(&source)?;
    Ok(source)
}

fn syntax(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("syntax", message, None)
}

fn resource(message: &str) -> Diagnostic {
    Diagnostic::new("resource_limit", message, None)
}

fn size(source: &str) -> Result<(), Diagnostic> {
    if source.len() > MAX_INPUT_BYTES {
        return Err(resource("Encoding exceeds 8 MiB"));
    }
    Ok(())
}

fn unsigned(text: &str) -> Result<Value, Diagnostic> {
    if text.is_empty()
        || !text.bytes().all(|byte| byte.is_ascii_digit())
        || (text.len() > 1 && text.starts_with('0'))
    {
        return Err(syntax(
            "Expected an unsigned decimal integer without leading zeros",
        ));
    }
    text.parse::<u64>()
        .map(Value::from)
        .map_err(|_| syntax("Integer exceeds u64"))
}

fn quoted(value: &str) -> String {
    serde_json::to_string(value)
        .expect("string value")
        .replace('\u{7f}', "\\u007f")
        .replace('\u{85}', "\\u0085")
        .replace('\u{fffe}', "\\ufffe")
        .replace('\u{ffff}', "\\uffff")
        .replace('\u{2028}', "\\u2028")
        .replace('\u{2029}', "\\u2029")
}

pub(crate) fn validate_value(value: &Value) -> Result<(), Diagnostic> {
    let mut pending = vec![(value, 1)];
    let mut nodes = 0;
    let mut bytes = 0;
    while let Some((value, depth)) = pending.pop() {
        nodes += 1;
        if depth > MAX_VALUE_DEPTH || nodes > MAX_VALUE_NODES {
            return Err(resource("Value tree exceeds 96 levels or 262144 nodes"));
        }
        match value {
            Value::Null | Value::Bool(_) => {}
            Value::Number(number) if number.as_u64().is_some() => {}
            Value::Number(_) => {
                return Err(syntax(
                    "SMSL values require unsigned integers, not signed or floating-point numbers",
                ));
            }
            Value::String(text) => bytes += text.len(),
            Value::Array(items) => pending.extend(items.iter().map(|item| (item, depth + 1))),
            Value::Object(fields) => {
                bytes += fields.keys().map(String::len).sum::<usize>();
                pending.extend(fields.values().map(|item| (item, depth + 1)));
            }
        }
        if bytes > MAX_INPUT_BYTES {
            return Err(resource("Value strings exceed 8 MiB"));
        }
    }
    Ok(())
}
