use std::collections::BTreeMap;

use serde::{Deserialize, Serialize};

pub(crate) use crate::encoding::{unique_map, unique_optional_map};
use crate::language::model::EventRef;
pub use crate::language::semantics::{BranchSnapshot, Context, Event};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Snapshot {
    pub version: String,
    pub definition: String,
    pub revision: String,
    pub generation: u64,
    pub context: Context,
    pub step: u64,
    #[serde(deserialize_with = "Option::deserialize")]
    pub time: Option<u64>,
    #[serde(deserialize_with = "unique_map")]
    pub branches: BTreeMap<String, BranchSnapshot>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Step {
    pub snapshot: Snapshot,
    pub derived_events: Vec<Event>,
    pub changed: Vec<String>,
    pub event_ref: EventRef,
}
