use std::collections::BTreeMap;

use serde::Serialize;

use crate::{Diagnostic, model::Datum, runtime::Context};

pub use crate::finite::Operator;

/// Traversal bounds and the permitted atomic operation batches.
#[derive(Clone, Debug)]
pub struct Options {
    pub operators: Option<Vec<Operator>>,
    pub max_states: usize,
}

impl Default for Options {
    fn default() -> Self {
        Self {
            operators: None,
            max_states: 100,
        }
    }
}

#[derive(Clone, Debug, Serialize)]
pub struct Excluded {
    pub actions: usize,
    pub reactions: usize,
}

#[derive(Clone, Debug, Serialize)]
pub struct Support {
    pub supported: bool,
    pub scope: &'static str,
    pub operators: Vec<Operator>,
    pub excluded: Excluded,
    pub reasons: Vec<Diagnostic>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Basis {
    pub revision: String,
    pub context: Context,
    pub generation: u64,
    pub step: u64,
}

#[derive(Clone, Debug, Serialize)]
pub struct Branch {
    pub facts: BTreeMap<String, Datum>,
    pub state: Option<String>,
    pub memberships: BTreeMap<String, Datum>,
    pub labels: BTreeMap<String, Datum>,
    pub activation: Option<Datum>,
}

#[derive(Clone, Debug, Serialize)]
pub struct Node {
    pub id: String,
    pub branches: BTreeMap<String, Branch>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Edge {
    pub source: String,
    pub target: String,
    pub operator: String,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Boundary {
    pub source: String,
    pub operator: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum Status {
    Complete,
    StateLimit,
    Unsupported,
}

#[derive(Clone, Debug, Serialize)]
pub struct Graph {
    pub status: Status,
    pub support: Support,
    pub max_states: usize,
    pub basis: Basis,
    pub initial: Option<String>,
    pub nodes: Vec<Node>,
    pub edges: Vec<Edge>,
    pub boundary: Vec<Boundary>,
}
