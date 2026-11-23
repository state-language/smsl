use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};
use serde_json::Value;

use super::{Freshness, Port, error};
use crate::Diagnostic;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Basis {
    pub revision: String,
    pub tick: u64,
}

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Record {
    pub schema: String,
    pub value: Value,
    pub basis: Basis,
    pub sequence: u64,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub unit: Option<String>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub frame: Option<String>,
}

pub type Records = BTreeMap<String, Record>;

#[derive(Clone, Debug, Deserialize, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Invocation {
    pub request_id: String,
    pub block: String,
    pub provider: String,
    pub config: Value,
    pub contract: String,
    pub inputs: Records,
    pub basis: Basis,
    pub allow_effects: bool,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TraceEntry {
    pub index: u64,
    pub kind: String,
    pub basis: Basis,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub invocation_basis: Option<Basis>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub block: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub request_id: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub endpoint: Option<String>,
    #[serde(skip_serializing_if = "Option::is_none")]
    pub detail: Option<Value>,
}

#[derive(Clone, Debug, PartialEq, Serialize)]
pub struct TracePage {
    pub entries: Vec<TraceEntry>,
    pub next: u64,
    pub first: u64,
    pub truncated: bool,
}

impl Basis {
    pub(super) fn validate(&self) -> Result<(), Diagnostic> {
        if self.revision.trim().is_empty() {
            return Err(error("invalid_basis", "Revision must not be empty"));
        }
        Ok(())
    }
}

impl Record {
    pub fn validate(&self, port: &Port, basis: &Basis) -> Result<(), Diagnostic> {
        if self.schema != port.schema || self.unit != port.unit || self.frame != port.frame {
            return Err(error(
                "record_type",
                "Record schema, unit or frame differs from its port",
            ));
        }
        self.basis.validate()?;
        if self.basis.revision != basis.revision
            || self.basis.tick > basis.tick
            || (port.freshness == Freshness::Current && self.basis.tick != basis.tick)
        {
            return Err(error(
                "stale_record",
                "Record does not satisfy the port's revision and tick basis",
            ));
        }
        if self.schema.ends_with("[]") {
            if !self.value.is_array() {
                return Err(error(
                    "record_value",
                    "Array schema requires an array payload",
                ));
            }
        } else if !self.value.is_object() {
            return Err(error("record_value", "Record payload must be an object"));
        }
        crate::encoding::validate_value(&self.value)
    }
}
