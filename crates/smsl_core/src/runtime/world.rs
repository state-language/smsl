mod archive;
mod changes;
mod commands;
mod transaction;

use std::{collections::BTreeMap, sync::Arc};

use serde::{Deserialize, Serialize};

use crate::{Diagnostic, Machine, model::EventRef};

use super::{
    Context, Event, RuntimeState, RuntimeStep, Snapshot, evolution::Migration, validation::error,
};

pub use commands::ActionTicket;
pub(crate) use transaction::ExecutionCheckpoint;
type Operations = BTreeMap<String, String>;

#[derive(Clone, Debug)]
struct Scope {
    initial_revision: String,
    machine: Arc<Machine>,
    state: RuntimeState,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
pub struct EventRecord {
    pub reference: EventRef,
    pub revision: String,
    pub event: Event,
    #[serde(deserialize_with = "super::types::unique_optional_map")]
    pub operations: Option<Operations>,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(deny_unknown_fields)]
struct RevisionRecord {
    scope: String,
    base_revision: String,
    base_generation: u64,
    base_step: u64,
    revision: String,
    migration: Migration,
}

#[derive(Clone, Debug, Deserialize, Serialize)]
#[serde(tag = "kind", rename_all = "snake_case", deny_unknown_fields)]
enum Record {
    Event(EventRecord),
    Revision(RevisionRecord),
}

#[derive(Clone, Debug, Default)]
pub struct World {
    instances: BTreeMap<String, String>,
    scopes: BTreeMap<String, Scope>,
    definitions: BTreeMap<String, Arc<Machine>>,
    records: Vec<Record>,
    references: BTreeMap<(String, u64), usize>,
    pending: BTreeMap<u64, ActionTicket>,
    next_ticket: u64,
}

impl World {
    pub fn new() -> Self {
        Self::default()
    }

    pub fn spawn(
        &mut self,
        name: impl Into<String>,
        machine: Arc<Machine>,
        context: Context,
    ) -> Result<(), Diagnostic> {
        let name = name.into();
        if name.is_empty() || self.instances.contains_key(&name) {
            return Err(error(
                "invalid_instance",
                "Instance name must be nonempty and unused",
            )
            .at_path("/instance"));
        }
        let scope = context.scope.clone();
        self.add_scope(machine, context)?;
        self.instances.insert(name, scope);
        Ok(())
    }

    pub fn remove(&mut self, name: &str) -> Result<(), Diagnostic> {
        let scope = self
            .instances
            .get(name)
            .ok_or_else(|| missing_instance(name))?;
        self.quiescent(scope)?;
        self.instances.remove(name);
        Ok(())
    }

    pub fn reset(&mut self, name: &str, context: Context) -> Result<(), Diagnostic> {
        let old = self
            .instances
            .get(name)
            .ok_or_else(|| missing_instance(name))?;
        self.quiescent(old)?;
        let machine = Arc::clone(&self.scopes[old].machine);
        let scope = context.scope.clone();
        self.add_scope(machine, context)?;
        self.instances.insert(name.into(), scope);
        Ok(())
    }

    pub fn state(&self, name: &str) -> Option<&RuntimeState> {
        Some(&self.scopes.get(self.instances.get(name)?)?.state)
    }
    pub fn machine(&self, name: &str) -> Option<&Arc<Machine>> {
        Some(&self.scopes.get(self.instances.get(name)?)?.machine)
    }
    pub fn instance_names(&self) -> impl Iterator<Item = &str> {
        self.instances.keys().map(String::as_str)
    }
    pub fn record_count(&self) -> usize {
        self.records.len()
    }
    pub fn definition_count(&self) -> usize {
        self.definitions.len()
    }

    pub fn snapshot(&self, name: &str) -> Result<Snapshot, Diagnostic> {
        Ok(self
            .state(name)
            .ok_or_else(|| missing_instance(name))?
            .snapshot())
    }

    pub fn resolve_record(&self, reference: &EventRef) -> Option<&EventRecord> {
        match &self.records[*self
            .references
            .get(&(reference.scope.clone(), reference.step))?]
        {
            Record::Event(record) => Some(record),
            Record::Revision(_) => None,
        }
    }

    pub fn resolve(&self, reference: &EventRef) -> Option<&Event> {
        self.resolve_record(reference).map(|record| &record.event)
    }

    pub fn advance(&mut self, name: &str, event: &Event) -> Result<RuntimeStep, Diagnostic> {
        let scope = self
            .instances
            .get(name)
            .ok_or_else(|| missing_instance(name))?
            .clone();
        self.consume(&scope, event, None)
    }

    pub fn confirm(
        &mut self,
        name: &str,
        event: &Event,
        operations: &Operations,
    ) -> Result<RuntimeStep, Diagnostic> {
        let scope = self
            .instances
            .get(name)
            .ok_or_else(|| missing_instance(name))?
            .clone();
        self.consume(&scope, event, Some(operations))
    }

    fn add_scope(&mut self, machine: Arc<Machine>, context: Context) -> Result<(), Diagnostic> {
        if self.scopes.contains_key(&context.scope) {
            return Err(error(
                "invalid_context",
                "A World scope cannot be reused, including after removal or reset",
            )
            .at_path("/context/scope"));
        }
        let state = RuntimeState::initial(&machine, context)?;
        let initial_revision = machine.revision().to_string();
        self.definitions
            .entry(initial_revision.clone())
            .or_insert_with(|| Arc::clone(&machine));
        self.scopes.insert(
            state.context.scope.clone(),
            Scope {
                initial_revision,
                machine,
                state,
            },
        );
        Ok(())
    }

    fn consume(
        &mut self,
        name: &str,
        event: &Event,
        operations: Option<&Operations>,
    ) -> Result<RuntimeStep, Diagnostic> {
        let scope = self
            .scopes
            .get(name)
            .ok_or_else(|| error("invalid_world", "Unknown event scope"))?;
        let resolve = |reference: &EventRef| self.resolve(reference).is_some();
        let step = match operations {
            Some(operations) => {
                scope
                    .state
                    .confirm_with(&scope.machine, event, operations, &resolve)
            }
            None => scope.state.advance_with(&scope.machine, event, &resolve),
        }?;
        let revision = scope.machine.revision().to_string();
        self.scopes.get_mut(name).expect("validated scope").state = step.state.clone();
        self.references.insert(
            (step.event_ref.scope.clone(), step.event_ref.step),
            self.records.len(),
        );
        self.records.push(Record::Event(EventRecord {
            reference: step.event_ref.clone(),
            revision,
            event: event.clone(),
            operations: operations.cloned(),
        }));
        Ok(step)
    }
}

fn missing_instance(name: &str) -> Diagnostic {
    error("invalid_instance", format!("Unknown instance {name:?}")).at_path("/instance")
}
