use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

use crate::model::{Datum, Expr, UIntRange, Value};
use crate::runtime::{Context, types::unique_map};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct SnapshotBasis {
    pub context: Context,
    pub generation: u64,
    pub step: u64,
    #[serde(deserialize_with = "Option::deserialize")]
    pub time: Option<u64>,
    pub fingerprint: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct AbstractionValue {
    pub revision: String,
    pub basis: SnapshotBasis,
    pub branch: String,
    pub abstraction: String,
    #[serde(deserialize_with = "unique_map")]
    pub facts: BTreeMap<String, Datum>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct ConcreteInput {
    pub branch: String,
    pub field: String,
    pub values: Vec<Value>,
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub range: Option<UIntRange>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Concretization {
    pub version: String,
    pub revision: String,
    pub branch: String,
    pub abstraction: String,
    #[serde(deserialize_with = "unique_map")]
    pub requirements: BTreeMap<String, Value>,
    #[serde(deserialize_with = "Option::deserialize")]
    pub basis: Option<SnapshotBasis>,
    pub condition: Expr,
    pub inputs: Vec<ConcreteInput>,
}

impl Concretization {
    pub fn validate_record(&self) -> Result<(), crate::Diagnostic> {
        if self.version != "1" {
            return Err(crate::Diagnostic::new(
                "invalid_concretization",
                "Unsupported concretization format version",
                None,
            ));
        }
        crate::language::limits::expression(&self.condition)
    }
}
