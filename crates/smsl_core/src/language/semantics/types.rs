use std::{collections::BTreeMap, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::{
    encoding::unique_map,
    language::model::{Datum, EventRef},
};

pub(crate) type BranchValues = BTreeMap<String, Arc<FactoredBranch>>;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Context {
    pub scope: String,
    pub subject: String,
    pub clock: String,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Event {
    pub name: String,
    pub time: u64,
    pub context: Context,
    #[serde(deserialize_with = "Option::deserialize")]
    pub cause: Option<EventRef>,
    #[serde(deserialize_with = "unique_map")]
    pub payload: BTreeMap<String, Datum>,
}

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct BranchSnapshot {
    #[serde(deserialize_with = "Option::deserialize")]
    pub state: Option<String>,
    #[serde(
        default,
        skip_serializing_if = "BTreeMap::is_empty",
        deserialize_with = "unique_map"
    )]
    pub memberships: BTreeMap<String, Datum>,
    #[serde(deserialize_with = "unique_map")]
    pub facts: BTreeMap<String, Datum>,
    #[serde(deserialize_with = "unique_map")]
    pub labels: BTreeMap<String, Datum>,
    #[serde(deserialize_with = "unique_map")]
    pub data: BTreeMap<String, Datum>,
}

// The reserved finite fact `state` is the storage for optional monitor notation.
// Public snapshots project it into their historical `state` field.
#[derive(Debug)]
pub(crate) struct FactoredBranch {
    pub facts: BTreeMap<String, Datum>,
    pub labels: BTreeMap<String, Datum>,
    pub memberships: BTreeMap<String, Datum>,
    pub data: BTreeMap<String, Datum>,
    snapshot: std::sync::OnceLock<BranchSnapshot>,
}

impl Clone for FactoredBranch {
    fn clone(&self) -> Self {
        Self {
            facts: self.facts.clone(),
            labels: self.labels.clone(),
            memberships: self.memberships.clone(),
            data: self.data.clone(),
            snapshot: std::sync::OnceLock::new(),
        }
    }
}
impl PartialEq for FactoredBranch {
    fn eq(&self, other: &Self) -> bool {
        self.facts == other.facts
            && self.labels == other.labels
            && self.memberships == other.memberships
            && self.data == other.data
    }
}
impl Eq for FactoredBranch {}

impl FactoredBranch {
    pub fn from_snapshot(mut snapshot: BranchSnapshot) -> Self {
        if let Some(state) = snapshot.state {
            snapshot.facts.insert("state".into(), Datum::String(state));
        }
        Self {
            facts: snapshot.facts,
            labels: snapshot.labels,
            memberships: snapshot.memberships,
            data: snapshot.data,
            snapshot: std::sync::OnceLock::new(),
        }
    }
    pub fn control_state(&self) -> Option<&str> {
        match self.facts.get("state") {
            Some(Datum::String(name)) => Some(name),
            _ => None,
        }
    }
    pub fn snapshot(&self) -> &BranchSnapshot {
        self.snapshot.get_or_init(|| {
            let mut facts = self.facts.clone();
            facts.remove("state");
            BranchSnapshot {
                state: self.control_state().map(str::to_owned),
                facts,
                labels: self.labels.clone(),
                memberships: self.memberships.clone(),
                data: self.data.clone(),
            }
        })
    }
    pub fn edit(value: &mut Arc<Self>) -> &mut Self {
        let value = Arc::make_mut(value);
        value.snapshot.take();
        value
    }
}
