//! Finite, deterministic planning over guarded operations and monitor edges.
//!
//! Search uses immutable trials and never dispatches actions or consumes live events.
//! Reactions are not operators. Time, payloads, evidence and retained data may not
//! influence the searched state. Explicit batches coordinate several branches.

mod request;
mod search;

use serde::{Deserialize, Serialize};

use crate::{
    Diagnostic, Machine,
    finite::{Domain, planning_error, validation},
    language::semantics::Evaluation,
    model::Expr,
    runtime::RuntimeState,
};

pub use request::request;
pub use search::{SearchLimits, SearchOutcome, SearchResult};

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Goal {
    pub branch: String,
    pub expression: Expr,
}

pub use crate::finite::Operator;

/// A validated finite domain. Operators are tried in the supplied order.
pub struct Planner<'a> {
    machine: &'a Machine,
    goal: Goal,
    domain: Domain<'a>,
}

impl<'a> Planner<'a> {
    /// Enumerate each declared operation/monitor edge as a separate operator.
    /// Use `new` for atomic batches that coordinate several branches.
    pub fn local(machine: &'a Machine, goal: Goal) -> Result<Self, Diagnostic> {
        Self::new(machine, goal, Domain::local(machine))
    }

    pub fn new(
        machine: &'a Machine,
        goal: Goal,
        operators: Vec<Operator>,
    ) -> Result<Self, Diagnostic> {
        validation::goal(machine, &goal)?;
        let (domain, reasons) = Domain::prepare(machine, operators)?;
        if let Some(reason) = reasons.into_iter().next() {
            return Err(planning_error(reason));
        }
        Ok(Self {
            machine,
            goal,
            domain,
        })
    }

    pub fn operators(&self) -> &[Operator] {
        &self.domain.operators
    }

    pub fn goal_satisfied(&self, state: &RuntimeState) -> Result<bool, Diagnostic> {
        self.check_state(state)?;
        self.goal_holds(state)
    }

    /// Return applicable operators, including invariant and postcondition checks.
    pub fn enabled_operations(&self, state: &RuntimeState) -> Result<Vec<&Operator>, Diagnostic> {
        self.check_state(state)?;
        let mut enabled = Vec::new();
        for operator in &self.domain.operators {
            if self.apply(state, operator)?.is_some() {
                enabled.push(operator);
            }
        }
        Ok(enabled)
    }

    /// Replay a plan against the supplied basis and require its final goal.
    /// The returned state is hypothetical; the basis is never changed.
    pub fn validate_plan(
        &self,
        initial: &RuntimeState,
        plan: &[String],
    ) -> Result<RuntimeState, Diagnostic> {
        self.check_state(initial)?;
        let mut state = initial.clone();
        for (index, name) in plan.iter().enumerate() {
            let operator = self
                .domain
                .operators
                .iter()
                .find(|operator| operator.name == *name)
                .ok_or_else(|| invalid(format!("Unknown plan operator {name:?}")))?;
            state = self.apply(&state, operator)?.ok_or_else(|| {
                invalid(format!("Plan operator {name:?} is not applicable"))
                    .at_path(format!("/plan/{index}"))
            })?;
        }
        if !self.goal_holds(&state)? {
            return Err(invalid("Plan does not establish its goal"));
        }
        Ok(state)
    }

    fn check_state(&self, state: &RuntimeState) -> Result<(), Diagnostic> {
        self.domain.check_state(state)
    }

    fn goal_holds(&self, state: &RuntimeState) -> Result<bool, Diagnostic> {
        let branch = &self.machine.document().branches[self
            .machine
            .branch_id(&self.goal.branch)
            .expect("validated goal branch")];
        Evaluation::state(&state.branches, branch).boolean(&self.goal.expression)
    }

    fn apply(
        &self,
        state: &RuntimeState,
        operator: &Operator,
    ) -> Result<Option<RuntimeState>, Diagnostic> {
        self.domain.apply(state, operator)
    }
}

fn invalid(message: impl Into<String>) -> Diagnostic {
    Diagnostic::new("invalid_planning", message, None)
}
