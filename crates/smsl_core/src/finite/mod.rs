use std::collections::{BTreeMap, BTreeSet};

use serde::{Deserialize, Serialize};

use crate::{
    Diagnostic, Machine,
    language::semantics::{Evaluation, OperationSelection},
    runtime::{Event, RuntimeState},
};

pub(crate) mod validation;

#[derive(Clone, Debug, Deserialize, Eq, PartialEq, Serialize)]
#[serde(deny_unknown_fields)]
pub struct Operator {
    pub name: String,
    pub operations: BTreeMap<String, String>,
}

pub(crate) struct Domain<'a> {
    pub machine: &'a Machine,
    pub operators: Vec<Operator>,
    event_name: String,
}

impl<'a> Domain<'a> {
    pub fn local(machine: &Machine) -> Vec<Operator> {
        let mut operators = Vec::new();
        for (index, branch) in machine.document().branches.iter().enumerate() {
            let names: BTreeSet<_> = machine.programs[index]
                .operations
                .iter()
                .map(|operation| operation.name.clone())
                .collect();
            for name in names {
                operators.push(Operator {
                    name: format!("{}::{name}", branch.name),
                    operations: BTreeMap::from([(branch.name.clone(), name)]),
                });
            }
        }
        operators
    }

    pub fn prepare(
        machine: &'a Machine,
        operators: Vec<Operator>,
    ) -> Result<(Self, Vec<Diagnostic>), Diagnostic> {
        validation::operators(machine, &operators)?;
        let reasons = validation::restrictions(machine, &operators);
        let mut event_name = "__planning".to_owned();
        while machine
            .document()
            .events
            .iter()
            .any(|event| event.name == event_name)
        {
            event_name.push('_');
        }
        Ok((
            Self {
                machine,
                operators,
                event_name,
            },
            reasons,
        ))
    }

    pub fn check_state(&self, state: &RuntimeState) -> Result<(), Diagnostic> {
        state.check_machine(self.machine)?;
        state.check_standalone()?;
        if let Some(reason) = validation::unknowns(state).into_iter().next() {
            return Err(planning_error(reason));
        }
        Ok(())
    }
    pub(crate) fn apply(
        &self,
        state: &RuntimeState,
        operator: &Operator,
    ) -> Result<Option<RuntimeState>, Diagnostic> {
        for (name, operation) in &operator.operations {
            let index = self
                .machine
                .branch_id(name)
                .expect("validated operator branch");
            let branch = &self.machine.document().branches[index];
            let evaluation = Evaluation::state(&state.branches, branch);
            match evaluation
                .select_operation(&self.machine.programs[index].operations, operation)?
            {
                OperationSelection::Enabled(_) => {}
                OperationSelection::Disabled | OperationSelection::Unknown => return Ok(None),
                OperationSelection::Ambiguous => {
                    return Err(Diagnostic::new(
                        "ambiguous_transition",
                        "Multiple enabled operation definitions",
                        None,
                    ));
                }
            }
        }
        let event = Event {
            name: self.event_name.clone(),
            time: state.time().unwrap_or(0),
            context: state.context().clone(),
            cause: None,
            payload: BTreeMap::new(),
        };
        match state.confirm(self.machine, &event, &operator.operations) {
            Ok(step) => {
                self.check_state(&step.state)?;
                Ok(Some(step.state))
            }
            // Enabled operations may still fail their postcondition or a global
            // invariant. Other failures, including arithmetic errors, are defects.
            Err(error)
                if error.code == "invalid_operation"
                    || (error.code == "invalid_snapshot"
                        && error
                            .path
                            .as_deref()
                            .is_some_and(|path| path.contains("/invariants/"))) =>
            {
                Ok(None)
            }
            Err(error) => Err(error),
        }
    }
}

pub(crate) fn planning_error(mut diagnostic: Diagnostic) -> Diagnostic {
    diagnostic.code = "invalid_planning".into();
    diagnostic
}

pub(crate) fn identity(state: &RuntimeState) -> Result<Vec<u8>, Diagnostic> {
    let facts: Vec<_> = state
        .branches
        .iter()
        .map(|(name, branch)| (name, &branch.facts))
        .collect();
    serde_json::to_vec(&facts).map_err(|error| {
        Diagnostic::new(
            "invalid_planning",
            format!("Cannot encode finite search state: {error}"),
            None,
        )
    })
}
