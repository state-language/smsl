mod build;
mod export;
mod traversal;

use std::collections::HashMap;

use serde::Serialize;

use crate::language::model::{Assignment, Expr, Update};
use crate::{Diagnostic, Machine};

#[derive(Debug, Serialize)]
pub struct Graph {
    nodes: Vec<Node>,
    edges: Vec<Edge>,
    dependencies: Vec<Dependency>,
    #[serde(skip)]
    names: HashMap<String, HashMap<String, usize>>,
    #[serde(skip)]
    outgoing: Vec<Vec<usize>>,
    #[serde(skip)]
    incoming: Vec<Vec<usize>>,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Node {
    pub id: usize,
    pub branch: String,
    pub state: String,
    pub values: Vec<Assignment>,
    pub initial: bool,
    pub activating: bool,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Edge {
    pub id: usize,
    pub source: usize,
    pub target: usize,
    pub kind: EdgeKind,
    pub label: String,
    pub guard: Option<Expr>,
    pub updates: Vec<Update>,
    pub emit: Vec<String>,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum EdgeKind {
    Operation,
    Reaction,
}

#[derive(Clone, Debug, Eq, PartialEq, Serialize)]
pub struct Dependency {
    pub source: String,
    pub target: String,
    pub kind: DependencyKind,
    pub name: String,
}

#[derive(Clone, Copy, Debug, Eq, PartialEq, Serialize)]
#[serde(rename_all = "snake_case")]
pub enum DependencyKind {
    Binding,
    Read,
}

impl Machine {
    pub fn graph(&self) -> Graph {
        Graph::from_document(self.document())
    }
}

impl Graph {
    pub fn nodes(&self) -> &[Node] {
        &self.nodes
    }

    pub fn edges(&self) -> &[Edge] {
        &self.edges
    }

    pub fn dependencies(&self) -> &[Dependency] {
        &self.dependencies
    }

    pub fn node(&self, branch: &str, state: &str) -> Result<&Node, Diagnostic> {
        self.names
            .get(branch)
            .and_then(|states| states.get(state))
            .map(|id| &self.nodes[*id])
            .ok_or_else(|| {
                Diagnostic::new(
                    "unknown_reference",
                    format!("Unknown graph node {branch:?}/{state:?}"),
                    None,
                )
            })
    }

    fn validate_node(&self, id: usize) -> Result<(), Diagnostic> {
        if id >= self.nodes.len() {
            return Err(Diagnostic::new(
                "unknown_reference",
                format!("Unknown graph node ID {id}"),
                None,
            ));
        }
        Ok(())
    }
}
