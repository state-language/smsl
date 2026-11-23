use super::super::{Block, Session, Status, error};
use crate::{
    Diagnostic,
    language::semantics::{Evaluation, OperationSelection},
    runtime::{Event, Step},
};
use std::collections::BTreeMap;

impl Session {
    pub(super) fn hook(&mut self, id: &str, steps: &mut Vec<Step>) -> Result<(), Diagnostic> {
        let Block::Operation {
            instance,
            operations,
        } = self.definition.blocks[id].clone()
        else {
            unreachable!("validated lifecycle operation")
        };
        let state = self.world.state(&instance).expect("validated instance");
        let time = state
            .time()
            .into_iter()
            .chain(self.times.get(&instance).copied())
            .max()
            .ok_or_else(|| {
                error(
                    "invalid_event",
                    "Lifecycle hook requires an execution clock",
                )
            })?;
        let status = self.perform_operation(
            &instance,
            &operations,
            &BTreeMap::from([(instance.clone(), time)]),
            steps,
        )?;
        if status != Status::Success {
            return Err(error(
                "invalid_execution",
                "Lifecycle operation must be applicable",
            ));
        }
        self.progress.get_mut(id).expect("hook block").status = status;
        Ok(())
    }

    pub(super) fn perform_operation(
        &mut self,
        instance: &str,
        operations: &BTreeMap<String, String>,
        times: &BTreeMap<String, u64>,
        steps: &mut Vec<Step>,
    ) -> Result<Status, Diagnostic> {
        let machine = self.world.machine(instance).expect("validated instance");
        let state = self.world.state(instance).expect("validated instance");
        let mut any_unknown = false;
        for (branch, name) in operations {
            let index = machine.branch_id(branch).expect("validated branch");
            let evaluation =
                Evaluation::state(&state.branches, &machine.document().branches[index]);
            match evaluation.select_operation(&machine.programs[index].operations, name)? {
                OperationSelection::Enabled(_) => {}
                OperationSelection::Disabled => return Ok(Status::Failure),
                OperationSelection::Unknown => any_unknown = true,
                OperationSelection::Ambiguous => {
                    return Err(error(
                        "ambiguous_transition",
                        "Multiple operation definitions are enabled",
                    ));
                }
            }
        }
        if any_unknown {
            return Ok(Status::Unknown);
        }
        let time = times.get(instance).copied().ok_or_else(|| {
            error(
                "invalid_event",
                format!("Tick requires a time for instance {instance:?}"),
            )
        })?;
        let mut name = "$execution.operation".to_owned();
        while machine
            .document()
            .events
            .iter()
            .any(|event| event.name == name)
        {
            name.push('_');
        }
        let event = Event {
            name,
            time,
            context: state.context().clone(),
            cause: None,
            payload: BTreeMap::new(),
        };
        steps.push(
            self.world
                .confirm(instance, &event, operations)?
                .into_step(),
        );
        Ok(Status::Success)
    }
}
