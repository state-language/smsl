use super::{
    model::{Abstraction, Datum, Document, Expr, State},
    validation::unknown,
};
use crate::Diagnostic;
use std::{collections::HashMap, sync::Arc};

#[derive(Debug)]
pub struct Machine {
    pub(super) document: Document,
    pub(crate) programs: Vec<super::compile::monitor::Program>,
    pub(super) canonical_source: Arc<str>,
    pub(super) names: HashMap<String, usize>,
    pub(super) branches: Vec<IndexedBranch>,
    pub(super) derived_order: Vec<(String, String)>,
    pub(super) revision: String,
}

#[derive(Debug)]
pub(super) struct IndexedBranch {
    pub(super) states: HashMap<String, usize>,
    pub(super) facts: HashMap<String, usize>,
    pub(super) operations: HashMap<String, usize>,
    pub(super) edges: Vec<HashMap<usize, usize>>,
    pub(super) values: Vec<Vec<Option<usize>>>,
    pub(super) initial: Option<usize>,
    pub(super) bindings: HashMap<usize, usize>,
}

impl Machine {
    pub fn revision(&self) -> &str {
        &self.revision
    }

    pub fn derived_order(&self) -> &[(String, String)] {
        &self.derived_order
    }

    pub fn initial_control_id(&self, branch: usize) -> Result<Option<usize>, Diagnostic> {
        Ok(self.indexed_branch(branch)?.initial)
    }

    pub fn canonical_source(&self) -> &Arc<str> {
        &self.canonical_source
    }

    pub fn document(&self) -> &Document {
        &self.document
    }

    pub fn abstraction(&self, branch: &str, name: &str) -> Result<&Abstraction, Diagnostic> {
        let branch = self.resolve_branch(branch)?;
        self.document.branches[branch]
            .abstractions
            .iter()
            .find(|abstraction| abstraction.name == name)
            .ok_or_else(|| unknown("abstraction", name, None))
    }

    pub fn branch_id(&self, name: &str) -> Option<usize> {
        self.names.get(name).copied()
    }

    pub fn state_id(&self, branch: usize, name: &str) -> Option<usize> {
        self.branches.get(branch)?.states.get(name).copied()
    }

    pub fn fact_id(&self, branch: usize, name: &str) -> Option<usize> {
        self.branches.get(branch)?.facts.get(name).copied()
    }

    pub fn operation_id(&self, branch: usize, name: &str) -> Option<usize> {
        self.branches.get(branch)?.operations.get(name).copied()
    }

    pub fn initial_id(&self, branch: usize) -> Result<usize, Diagnostic> {
        self.indexed_branch(branch)?.initial.ok_or_else(|| {
            Diagnostic::new(
                "no_control_state",
                "This branch has no named control states",
                None,
            )
        })
    }

    pub fn activating_id(&self, branch: usize) -> Result<Option<usize>, Diagnostic> {
        self.indexed_branch(branch)?;
        let definition = &self.document.branches[branch];
        let Some(expr) = &definition.activating else {
            return Ok(None);
        };
        let name = control_activation(expr).ok_or_else(|| {
            Diagnostic::new(
                "runtime_activation",
                "This activation predicate requires a runtime snapshot",
                None,
            )
        })?;
        self.state_id(branch, name)
            .map(Some)
            .ok_or_else(|| unknown("activation state", name, None))
    }

    pub fn value_id(&self, branch: usize, state: usize, fact: usize) -> Result<usize, Diagnostic> {
        self.indexed_branch(branch)?
            .values
            .get(state)
            .and_then(|values| values.get(fact))
            .copied()
            .flatten()
            .ok_or_else(|| {
                unknown(
                    "state constraint or fact index",
                    &format!("{state}/{fact}"),
                    None,
                )
            })
    }

    pub fn next_id(
        &self,
        branch: usize,
        state: usize,
        operation: usize,
    ) -> Result<Option<usize>, Diagnostic> {
        let branch = self.indexed_branch(branch)?;
        let edges = branch
            .edges
            .get(state)
            .ok_or_else(|| unknown("state index", &state.to_string(), None))?;
        if operation >= branch.operations.len() {
            return Err(unknown("operation index", &operation.to_string(), None));
        }
        Ok(edges.get(&operation).copied())
    }

    pub fn successor_id(
        &self,
        branch: usize,
        state: usize,
        operation: &str,
    ) -> Result<Option<usize>, Diagnostic> {
        let index = self.indexed_branch(branch)?;
        let operation = index
            .operations
            .get(operation)
            .copied()
            .ok_or_else(|| unknown("operation", operation, None))?;
        self.next_id(branch, state, operation)
    }

    pub fn successor(
        &self,
        branch: &str,
        state: &str,
        operation: &str,
    ) -> Result<Option<&State>, Diagnostic> {
        let branch = self.resolve_branch(branch)?;
        let state = self.resolve_state(branch, state)?;
        Ok(self
            .successor_id(branch, state, operation)?
            .map(|index| &self.document.branches[branch].states[index]))
    }

    pub fn activation(&self, branch: &str, state: &str) -> Result<bool, Diagnostic> {
        let branch = self.resolve_branch(branch)?;
        let state = self.resolve_state(branch, state)?;
        Ok(self.activating_id(branch)? == Some(state))
    }

    pub fn binding_value(
        &self,
        parent: &str,
        fact: &str,
        child_state: &str,
    ) -> Result<bool, Diagnostic> {
        let parent = self.resolve_branch(parent)?;
        let field = self
            .fact_id(parent, fact)
            .ok_or_else(|| unknown("fact", fact, None))?;
        let child = self.branches[parent]
            .bindings
            .get(&field)
            .copied()
            .ok_or_else(|| unknown("child binding for fact", fact, None))?;
        let state = self.resolve_state(child, child_state)?;
        Ok(self.activating_id(child)? == Some(state))
    }

    fn indexed_branch(&self, index: usize) -> Result<&IndexedBranch, Diagnostic> {
        self.branches
            .get(index)
            .ok_or_else(|| unknown("branch index", &index.to_string(), None))
    }

    fn resolve_branch(&self, name: &str) -> Result<usize, Diagnostic> {
        self.branch_id(name)
            .ok_or_else(|| unknown("branch", name, None))
    }

    fn resolve_state(&self, branch: usize, name: &str) -> Result<usize, Diagnostic> {
        self.state_id(branch, name)
            .ok_or_else(|| unknown("state", name, None))
    }
}

pub(crate) fn control_activation(expr: &Expr) -> Option<&str> {
    if let Expr::Eq { left, right } = expr {
        for (read, literal) in [
            (left.as_ref(), right.as_ref()),
            (right.as_ref(), left.as_ref()),
        ] {
            if let (
                Expr::Read { source, field },
                Expr::Literal {
                    value: Datum::String(name),
                },
            ) = (read, literal)
            {
                if source == "self" && field == "state" {
                    return Some(name);
                }
            }
        }
    }
    None
}
