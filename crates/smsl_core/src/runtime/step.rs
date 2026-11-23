use crate::language::semantics::{Evaluation, FactoredBranch, OperationSelection};

use std::collections::BTreeMap;

use crate::{
    Diagnostic, Machine,
    model::{Branch, EventRef, Update},
};

use super::{
    Context, Event, RuntimeState, Snapshot, Step,
    validation::{self, Resolver, error},
};

#[derive(Clone, Debug)]
pub struct RuntimeStep {
    pub state: RuntimeState,
    pub derived_events: Vec<Event>,
    pub changed: Vec<String>,
    pub event_ref: EventRef,
}

impl RuntimeStep {
    pub fn into_step(self) -> Step {
        Step {
            snapshot: self.state.into_snapshot(),
            derived_events: self.derived_events,
            changed: self.changed,
            event_ref: self.event_ref,
        }
    }
}

pub fn initial(machine: &Machine, context: Context) -> Result<Snapshot, Diagnostic> {
    Ok(RuntimeState::initial(machine, context)?.into_snapshot())
}

pub fn reset(
    machine: &Machine,
    snapshot: &Snapshot,
    context: Context,
) -> Result<Snapshot, Diagnostic> {
    Ok(RuntimeState::from_snapshot(machine, snapshot.clone())?
        .reset(machine, context)?
        .into_snapshot())
}

pub fn advance(machine: &Machine, snapshot: &Snapshot, event: &Event) -> Result<Step, Diagnostic> {
    Ok(RuntimeState::from_snapshot(machine, snapshot.clone())?
        .advance(machine, event)?
        .into_step())
}

pub fn confirm(
    machine: &Machine,
    snapshot: &Snapshot,
    event: &Event,
    operations: &BTreeMap<String, String>,
) -> Result<Step, Diagnostic> {
    Ok(RuntimeState::from_snapshot(machine, snapshot.clone())?
        .confirm(machine, event, operations)?
        .into_step())
}

impl RuntimeState {
    fn begin(
        &self,
        machine: &Machine,
        event: &Event,
        resolve: &Resolver<'_>,
    ) -> Result<(Self, EventRef), Diagnostic> {
        self.check_machine(machine)?;
        validation::event(machine, event, self, resolve)?;
        let mut next = self.clone();
        next.step = self
            .step
            .checked_add(1)
            .ok_or_else(|| error("invalid_event", "Consumed event step overflow"))?;
        next.time = Some(event.time);
        let reference = EventRef {
            scope: self.context.scope.clone(),
            step: next.step,
        };
        Ok((next, reference))
    }

    fn finish(
        &self,
        machine: &Machine,
        mut next: Self,
        event_ref: EventRef,
        derived_events: Vec<Event>,
        resolve: &Resolver<'_>,
    ) -> Result<RuntimeStep, Diagnostic> {
        next.recompute(machine)?;
        validation::validate_values(machine, &next, resolve)?;
        next.validate_constraints(machine)?;
        next.refresh_references();
        let changed = next
            .branches
            .iter()
            .filter(|(name, value)| self.branches.get(*name) != Some(*value))
            .map(|(name, _)| name.clone())
            .collect();
        Ok(RuntimeStep {
            state: next,
            derived_events,
            changed,
            event_ref,
        })
    }

    fn apply(
        &mut self,
        branch: &Branch,
        updates: &[Update],
        evaluation: &Evaluation<'_>,
        path: &str,
    ) -> Result<(), Diagnostic> {
        let current = FactoredBranch::edit(
            self.branches
                .get_mut(&branch.name)
                .expect("validated branch"),
        );
        for (index, update) in updates.iter().enumerate() {
            let value = evaluation
                .evaluate(&update.value)
                .map_err(|error| error.prepend_path(&format!("{path}/updates/{index}/value")))?;
            if let Some(field) = current.facts.get_mut(&update.field) {
                *field = value;
            } else if let Some(field) = current.data.get_mut(&update.field) {
                *field = value;
            } else {
                return Err(error(
                    "unknown_reference",
                    format!("Unknown update field {:?}", update.field),
                ));
            }
        }
        Ok(())
    }

    pub(crate) fn advance_with(
        &self,
        machine: &Machine,
        event: &Event,
        resolve: &Resolver<'_>,
    ) -> Result<RuntimeStep, Diagnostic> {
        let (mut next, event_ref) = self.begin(machine, event, resolve)?;
        let mut derived_events = Vec::new();
        for (branch_index, branch) in machine.document().branches.iter().enumerate() {
            let evaluation = Evaluation {
                event: Some(event),
                event_ref: Some(&event_ref),
                ..Evaluation::state(&self.branches, branch)
            };
            let reactions = machine.programs[branch_index]
                .reactions
                .iter()
                .map(|(reaction, path)| (reaction, format!("/branches/{branch_index}{path}")));
            let mut selected = None;
            for (reaction, path) in reactions {
                if reaction.event != event.name {
                    continue;
                }
                let enabled = reaction
                    .guard
                    .as_ref()
                    .map_or(Ok(true), |guard| evaluation.boolean(guard))
                    .map_err(|error| error.prepend_path(&format!("{path}/guard")))?;
                if !enabled {
                    continue;
                }
                if selected.is_some() {
                    return Err(error(
                        "ambiguous_transition",
                        format!(
                            "Event {:?} matches multiple rules in branch {:?}",
                            event.name, branch.name
                        ),
                    )
                    .at_path(format!(
                        "/branches/{}/reactions",
                        validation::key(&branch.name)
                    )));
                }
                selected = Some((reaction, path));
            }
            if let Some((reaction, path)) = selected {
                next.apply(branch, &reaction.updates, &evaluation, &path)?;
                derived_events.extend(reaction.emit.iter().map(|name| Event {
                    name: name.clone(),
                    time: event.time,
                    context: event.context.clone(),
                    cause: Some(event_ref.clone()),
                    payload: BTreeMap::new(),
                }));
            }
        }
        self.finish(machine, next, event_ref, derived_events, resolve)
    }

    pub(crate) fn confirm_with(
        &self,
        machine: &Machine,
        event: &Event,
        operations: &BTreeMap<String, String>,
        resolve: &Resolver<'_>,
    ) -> Result<RuntimeStep, Diagnostic> {
        if operations.is_empty() {
            return Err(error(
                "invalid_operation",
                "Confirmation requires at least one branch operation",
            )
            .at_path("/operations"));
        }
        let (mut next, event_ref) = self.begin(machine, event, resolve)?;
        let mut postconditions = Vec::new();
        for (name, operation) in operations {
            let operation_path = format!("/operations/{}", validation::key(name));
            let index = machine.branch_id(name).ok_or_else(|| {
                error("invalid_operation", format!("Unknown branch {name:?}"))
                    .at_path(&operation_path)
            })?;
            let branch = &machine.document().branches[index];
            let evaluation = Evaluation::state(&self.branches, branch);
            let declaration = match evaluation
                .select_operation(&machine.programs[index].operations, operation)?
            {
                OperationSelection::Enabled(declaration) => declaration,
                OperationSelection::Ambiguous => {
                    return Err(error(
                        "ambiguous_transition",
                        "Multiple enabled operation definitions",
                    )
                    .at_path(&operation_path));
                }
                OperationSelection::Disabled | OperationSelection::Unknown => {
                    return Err(
                        error("invalid_operation", "Operation has no enabled definition")
                            .at_path(&operation_path),
                    );
                }
            };
            next.apply(branch, &declaration.updates, &evaluation, &operation_path)?;
            if let Some(postcondition) = &declaration.postcondition {
                postconditions.push((branch, postcondition, operation_path));
            }
        }
        let result = self.finish(machine, next, event_ref, Vec::new(), resolve)?;
        for (branch, expression, path) in postconditions {
            if !Evaluation::state(&result.state.branches, branch).boolean(expression)? {
                return Err(error(
                    "invalid_operation",
                    "Operation postcondition is not established",
                )
                .at_path(path));
            }
        }
        Ok(result)
    }
}
