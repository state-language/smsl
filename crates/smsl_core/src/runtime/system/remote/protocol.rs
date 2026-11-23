use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::{Value, json};

use super::super::{Basis, Records, error};
use crate::Diagnostic;

pub const PROTOCOL: &str = "state-lang.provider/1";
pub const MAX_BYTES: usize = 1_048_576;

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Options {
    pub contract: String,
    pub profile: String,
    #[serde(default)]
    pub config: BTreeMap<String, Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct HttpRequest {
    pub method: &'static str,
    pub path: String,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub body: Option<Value>,
}

impl HttpRequest {
    pub(super) fn new(
        method: &'static str,
        path: String,
        body: Option<Value>,
    ) -> Result<Self, Diagnostic> {
        if let Some(body) = &body {
            crate::encoding::validate_value(body)?;
            if serde_json::to_vec(body).expect("remote request").len() > MAX_BYTES {
                return Err(error("resource_limit", "Provider request exceeds 1 MiB"));
            }
        }
        Ok(Self { method, path, body })
    }
}

pub(super) fn envelope(value: &Value) -> Result<(), Diagnostic> {
    if !value.is_object() {
        return Err(error("invalid_response", "Response must be an object"));
    }
    if serde_json::to_vec(value).expect("remote response").len() > MAX_BYTES {
        return Err(error("resource_limit", "Provider response exceeds 1 MiB"));
    }
    if value.get("protocol") != Some(&json!(PROTOCOL)) {
        return Err(error("protocol_mismatch", "Unsupported provider protocol"));
    }
    Ok(())
}

pub(super) fn string<'a>(value: &'a Value, field: &str) -> Result<&'a str, Diagnostic> {
    value
        .get(field)
        .and_then(Value::as_str)
        .filter(|value| !value.is_empty())
        .ok_or_else(|| {
            error(
                "invalid_response",
                format!("{field} must be a nonempty string"),
            )
        })
}

pub(super) fn integer(value: &Value, field: &str) -> Result<u64, Diagnostic> {
    value
        .get(field)
        .and_then(Value::as_u64)
        .ok_or_else(|| error("invalid_response", format!("{field} must be a u64")))
}

pub(super) fn outputs(value: &Value, basis: &Basis) -> Result<Records, Diagnostic> {
    let value = value
        .get("outputs")
        .ok_or_else(|| error("invalid_response", "Missing outputs"))?;
    if value
        .as_object()
        .is_none_or(|records| records.values().any(|record| record.get("value").is_none()))
    {
        return Err(error(
            "invalid_response",
            "Every output record requires a value",
        ));
    }
    let records: Records = serde_json::from_value(value.clone())
        .map_err(|cause| error("invalid_response", cause.to_string()))?;
    for record in records.values() {
        if record.schema.is_empty()
            || record.unit.as_ref().is_some_and(String::is_empty)
            || record.frame.as_ref().is_some_and(String::is_empty)
            || &record.basis != basis
        {
            return Err(error(
                "invalid_response",
                "Invalid output record or changed invocation basis",
            ));
        }
        crate::encoding::validate_value(&record.value)?;
    }
    Ok(records)
}

pub(super) fn remote_error(value: &Value) -> Result<Diagnostic, Diagnostic> {
    let value = value
        .get("error")
        .ok_or_else(|| error("invalid_response", "Missing operation error"))?;
    Ok(error(string(value, "code")?, string(value, "message")?))
}

pub(super) fn segment(value: &str) -> String {
    let mut result = String::new();
    for byte in value.bytes() {
        if byte.is_ascii_alphanumeric() || b"-_.!~*'()".contains(&byte) {
            result.push(char::from(byte));
        } else {
            use std::fmt::Write;
            write!(&mut result, "%{byte:02X}").expect("string write");
        }
    }
    result
}
